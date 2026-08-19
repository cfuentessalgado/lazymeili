use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use uuid::Uuid;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionColor {
    #[default]
    Violet,
    Blue,
    Cyan,
    Lime,
    Yellow,
    Orange,
    Red,
    Pink,
    Gray,
}

impl ConnectionColor {
    pub const ALL: [Self; 9] = [
        Self::Violet,
        Self::Blue,
        Self::Cyan,
        Self::Lime,
        Self::Yellow,
        Self::Orange,
        Self::Red,
        Self::Pink,
        Self::Gray,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Violet => "violet",
            Self::Blue => "blue",
            Self::Cyan => "cyan",
            Self::Lime => "lime",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Red => "red",
            Self::Pink => "pink",
            Self::Gray => "gray",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Application {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub has_api_key: bool,
    #[serde(default)]
    pub color: ConnectionColor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default = "config_version")]
    pub version: u32,
    #[serde(default)]
    pub applications: Vec<Application>,
    pub selected: Option<Uuid>,
}

const fn config_version() -> u32 {
    CONFIG_VERSION
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            applications: Vec::new(),
            selected: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub vault: PathBuf,
}

impl Paths {
    pub fn discover() -> anyhow::Result<Self> {
        let dirs = ProjectDirs::from("dev", "lazymeili", "lazymeili").ok_or_else(|| {
            anyhow::anyhow!("cannot determine the operating system config directory")
        })?;
        Ok(Self {
            config: dirs.config_dir().join("config.toml"),
            vault: dirs.config_dir().join("secrets.age"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> anyhow::Result<Config> {
        if !self.path.exists() {
            return Ok(Config::default());
        }
        let content = fs::read_to_string(&self.path)?;
        let config: Config = toml::from_str(&content)?;
        anyhow::ensure!(
            config.version <= CONFIG_VERSION,
            "config version {} is newer than LazyMeili supports",
            config.version
        );
        Ok(config)
    }

    pub fn save(&self, config: &Config) -> anyhow::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid config path"))?;
        fs::create_dir_all(parent)?;
        let mut temp = NamedTempFile::new_in(parent)?;
        set_private(temp.path())?;
        temp.write_all(toml::to_string_pretty(config)?.as_bytes())?;
        temp.as_file_mut().sync_all()?;
        temp.persist(&self.path).map_err(|error| error.error)?;
        set_private(&self.path)?;
        Ok(())
    }

    pub fn upsert(&self, config: &mut Config, app: Application) -> anyhow::Result<()> {
        if let Some(existing) = config
            .applications
            .iter_mut()
            .find(|item| item.id == app.id)
        {
            *existing = app;
        } else {
            config.applications.push(app);
        }
        self.save(config)
    }

    pub fn remove(&self, config: &mut Config, id: Uuid) -> anyhow::Result<()> {
        config.applications.retain(|app| app.id != id);
        if config.selected == Some(id) {
            config.selected = config.applications.first().map(|app| app.id);
        }
        self.save(config)
    }
}

pub fn normalize_url(value: &str) -> anyhow::Result<String> {
    let mut url = reqwest::Url::parse(value.trim())?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https"),
        "URL must use http or https"
    );
    anyhow::ensure!(url.host().is_some(), "URL must contain a host");
    url.set_query(None);
    url.set_fragment(None);
    let normalized = url.as_str().trim_end_matches('/').to_owned();
    Ok(normalized)
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
    fn round_trip_and_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(dir.path().join("config.toml"));
        let mut config = Config::default();
        let app = Application {
            id: Uuid::new_v4(),
            name: "Local".into(),
            url: "http://localhost:7700".into(),
            has_api_key: true,
            color: ConnectionColor::Red,
        };
        store.upsert(&mut config, app.clone()).unwrap();
        assert_eq!(store.load().unwrap().applications, vec![app]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.path().join("config.toml"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn old_configs_get_the_default_connection_color() {
        let id = Uuid::new_v4();
        let text = format!(
            "version = 1\nselected = \"{id}\"\n\n[[applications]]\nid = \"{id}\"\nname = \"Local\"\nurl = \"http://localhost:7700\"\nhas_api_key = false\n"
        );
        let config: Config = toml::from_str(&text).unwrap();
        assert_eq!(config.applications[0].color, ConnectionColor::Violet);
    }

    #[test]
    fn normalizes_connection_url() {
        assert_eq!(
            normalize_url("https://example.com/path/?x=1#fragment").unwrap(),
            "https://example.com/path"
        );
        assert!(normalize_url("file:///tmp/search").is_err());
    }
}
