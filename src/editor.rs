use std::{env, fs, io::Write, process::Command};

use serde_json::Value;
use tempfile::Builder;

use crate::terminal;

pub fn edit_json(terminal: &mut terminal::Tui, value: &Value) -> anyhow::Result<Option<Value>> {
    let mut file = Builder::new().prefix("mtui-").suffix(".json").tempfile()?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.as_file_mut().sync_all()?;

    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .map_err(|_| anyhow::anyhow!("set $VISUAL or $EDITOR to edit JSON"))?;
    let mut parts = editor.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("editor command is empty"))?;

    terminal::suspend(terminal)?;
    let status = Command::new(program).args(parts).arg(file.path()).status();
    let resume_result = terminal::resume(terminal);
    let status = status?;
    resume_result?;
    anyhow::ensure!(status.success(), "editor exited with status {status}");

    let text = fs::read_to_string(file.path())?;
    let edited: Value =
        serde_json::from_str(&text).map_err(|error| anyhow::anyhow!("invalid JSON: {error}"))?;
    if &edited == value {
        Ok(None)
    } else {
        Ok(Some(edited))
    }
}
