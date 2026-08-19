use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use age::secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const SERVICE: &str = "dev.lazymeili.lazymeili";
const LEGACY_SERVICE: &str = "dev.mtui.mtui";

pub trait SecretStore {
    fn get(&self, id: Uuid) -> anyhow::Result<Option<String>>;
    fn set(&mut self, id: Uuid, secret: &str) -> anyhow::Result<()>;
    fn delete(&mut self, id: Uuid) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct NativeStore;

impl NativeStore {
    #[cfg(target_os = "linux")]
    pub fn available() -> bool {
        let Ok(entry) = Self::entry(Uuid::nil()) else {
            return false;
        };
        matches!(entry.get_password(), Ok(_) | Err(keyring::Error::NoEntry))
    }

    fn entry(id: Uuid) -> anyhow::Result<keyring::Entry> {
        Self::entry_for(SERVICE, id)
    }

    fn entry_for(service: &str, id: Uuid) -> anyhow::Result<keyring::Entry> {
        keyring::Entry::new(service, &id.to_string())
            .map_err(|_| anyhow::anyhow!("native secret store is unavailable"))
    }

    fn delete_entry(entry: &keyring::Entry) -> anyhow::Result<()> {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => anyhow::bail!("cannot remove secret from native secret store"),
        }
    }
}

impl SecretStore for NativeStore {
    fn get(&self, id: Uuid) -> anyhow::Result<Option<String>> {
        let entry = Self::entry(id)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => {
                let legacy = Self::entry_for(LEGACY_SERVICE, id)?;
                match legacy.get_password() {
                    Ok(value) => {
                        entry
                            .set_password(&value)
                            .map_err(|_| anyhow::anyhow!("cannot migrate native secret"))?;
                        Self::delete_entry(&legacy)?;
                        Ok(Some(value))
                    }
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(_) => anyhow::bail!("cannot read from native secret store"),
                }
            }
            Err(_) => anyhow::bail!("cannot read from native secret store"),
        }
    }

    fn set(&mut self, id: Uuid, secret: &str) -> anyhow::Result<()> {
        Self::entry(id)?
            .set_password(secret)
            .map_err(|_| anyhow::anyhow!("cannot write to native secret store"))?;
        let legacy = Self::entry_for(LEGACY_SERVICE, id)?;
        Self::delete_entry(&legacy)
    }

    fn delete(&mut self, id: Uuid) -> anyhow::Result<()> {
        Self::delete_entry(&Self::entry(id)?)?;
        Self::delete_entry(&Self::entry_for(LEGACY_SERVICE, id)?)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct VaultData {
    secrets: BTreeMap<Uuid, String>,
}

pub struct AgeVault {
    path: PathBuf,
    passphrase: SecretString,
    data: VaultData,
}

impl std::fmt::Debug for AgeVault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgeVault")
            .field("path", &self.path)
            .field("secrets", &"[REDACTED]")
            .finish()
    }
}

impl AgeVault {
    pub fn open(path: PathBuf, passphrase: String) -> anyhow::Result<Self> {
        anyhow::ensure!(!passphrase.is_empty(), "vault passphrase cannot be empty");
        let passphrase = SecretString::from(passphrase);
        let data = if path.exists() {
            let encrypted = fs::read(&path)?;
            let decryptor = age::Decryptor::new(encrypted.as_slice())
                .map_err(|_| anyhow::anyhow!("invalid encrypted vault"))?;
            let identity = age::scrypt::Identity::new(passphrase.clone());
            let mut reader = decryptor
                .decrypt(std::iter::once(&identity as &dyn age::Identity))
                .map_err(|_| anyhow::anyhow!("vault unlock failed"))?;
            let mut plaintext = Vec::new();
            reader.read_to_end(&mut plaintext)?;
            let parsed = serde_json::from_slice(&plaintext)
                .map_err(|_| anyhow::anyhow!("vault contains invalid data"));
            plaintext.zeroize();
            parsed?
        } else {
            VaultData::default()
        };
        Ok(Self {
            path,
            passphrase,
            data,
        })
    }

    #[must_use]
    pub fn exists(path: &Path) -> bool {
        path.exists()
    }

    pub fn reset(path: &Path) -> anyhow::Result<()> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn save(&self) -> anyhow::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid vault path"))?;
        fs::create_dir_all(parent)?;
        let mut plaintext = serde_json::to_vec(&self.data)?;
        let encryptor = age::Encryptor::with_user_passphrase(self.passphrase.clone());
        let mut encrypted = Vec::new();
        {
            let mut writer = encryptor.wrap_output(&mut encrypted)?;
            writer.write_all(&plaintext)?;
            writer.finish()?;
        }
        plaintext.zeroize();
        let mut temp = NamedTempFile::new_in(parent)?;
        set_private(temp.path())?;
        temp.write_all(&encrypted)?;
        temp.as_file_mut().sync_all()?;
        temp.persist(&self.path).map_err(|error| error.error)?;
        set_private(&self.path)?;
        Ok(())
    }
}

impl SecretStore for AgeVault {
    fn get(&self, id: Uuid) -> anyhow::Result<Option<String>> {
        Ok(self.data.secrets.get(&id).cloned())
    }

    fn set(&mut self, id: Uuid, secret: &str) -> anyhow::Result<()> {
        self.data.secrets.insert(id, secret.to_owned());
        self.save()
    }

    fn delete(&mut self, id: Uuid) -> anyhow::Result<()> {
        if let Some(mut old) = self.data.secrets.remove(&id) {
            old.zeroize();
        }
        self.save()
    }
}

impl Drop for AgeVault {
    fn drop(&mut self) {
        for value in self.data.secrets.values_mut() {
            value.zeroize();
        }
    }
}

pub struct Secrets {
    native: NativeStore,
    fallback: Option<AgeVault>,
    cache: RefCell<BTreeMap<Uuid, Zeroizing<String>>>,
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Secrets")
            .field("cache", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl Secrets {
    #[must_use]
    pub fn new(fallback: Option<AgeVault>) -> Self {
        Self {
            native: NativeStore,
            fallback,
            cache: RefCell::new(BTreeMap::new()),
        }
    }
}

impl SecretStore for Secrets {
    fn get(&self, id: Uuid) -> anyhow::Result<Option<String>> {
        if let Some(secret) = self.cache.borrow().get(&id) {
            return Ok(Some(secret.to_string()));
        }
        let secret = match self.native.get(id) {
            Ok(Some(secret)) => Some(secret),
            Ok(None) => self
                .fallback
                .as_ref()
                .map_or(Ok(None), |vault| vault.get(id))?,
            Err(native_error) => self
                .fallback
                .as_ref()
                .map_or(Err(native_error), |vault| vault.get(id))?,
        };
        if let Some(value) = &secret {
            self.cache
                .borrow_mut()
                .insert(id, Zeroizing::new(value.clone()));
        }
        Ok(secret)
    }

    fn set(&mut self, id: Uuid, secret: &str) -> anyhow::Result<()> {
        self.native.set(id, secret).or_else(|native_error| {
            self.fallback
                .as_mut()
                .map_or(Err(native_error), |vault| vault.set(id, secret))
        })?;
        self.cache
            .borrow_mut()
            .insert(id, Zeroizing::new(secret.to_owned()));
        Ok(())
    }

    fn delete(&mut self, id: Uuid) -> anyhow::Result<()> {
        self.cache.borrow_mut().remove(&id);
        let native_result = self.native.delete(id);
        let fallback_result = self.fallback.as_mut().map(|vault| vault.delete(id));
        match (native_result, fallback_result) {
            (Ok(()), None | Some(Ok(()))) | (Err(_), Some(Ok(()))) => Ok(()),
            (Err(error), None) | (Err(error), Some(Err(_))) => Err(error),
            (Ok(()), Some(Err(error))) => Err(error),
        }
    }
}

fn set_private(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_round_trip_and_wrong_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.age");
        let id = Uuid::new_v4();
        let mut vault = AgeVault::open(path.clone(), "correct horse".into()).unwrap();
        vault.set(id, "master-key").unwrap();
        drop(vault);
        let vault = AgeVault::open(path.clone(), "correct horse".into()).unwrap();
        assert_eq!(vault.get(id).unwrap().as_deref(), Some("master-key"));
        assert!(AgeVault::open(path, "wrong".into()).is_err());
    }
}
