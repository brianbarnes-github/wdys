use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualKey(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelSize {
    BaseEn,
    SmallEn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub hotkey: VirtualKey,
    pub model: ModelSize,
    pub max_recording_secs: u32,
    pub idle_unload_secs: u32,
    pub double_press_ms: u32,
    pub min_hold_ms: u32,
    pub sounds: bool,
    pub start_with_windows: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: VirtualKey(0x78), // F9: the hotkey is blocked system-wide, so avoid a typing key
            model: ModelSize::BaseEn,
            max_recording_secs: 120,
            idle_unload_secs: 600,
            double_press_ms: 400,
            min_hold_ms: 300,
            sounds: true,
            start_with_windows: false,
        }
    }
}

/// Loads the config; a missing file yields defaults, a malformed one is a parse error.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| ConfigError::Parse(e.to_string())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(ConfigError::Io(e)),
    }
}

pub fn save(path: &Path, config: &Config) -> Result<(), ConfigError> {
    let text = toml::to_string_pretty(config).map_err(|e| ConfigError::Parse(e.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    Ok(())
}
