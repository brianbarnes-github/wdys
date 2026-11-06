use std::fs;

use wdys::config::{load, save, Config, ModelSize, VirtualKey};
use wdys::error::ConfigError;

#[test]
fn missing_file_yields_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let config = load(&dir.path().join("nope.toml")).unwrap();
    assert_eq!(config, Config::default());
    assert_eq!(Config::default().hotkey, VirtualKey(0x78));
}

#[test]
fn save_then_load_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let config = Config {
        hotkey: VirtualKey(0x7B),
        model: ModelSize::SmallEn,
        ..Config::default()
    };
    save(&path, &config).unwrap();
    assert_eq!(load(&path).unwrap(), config);
}

#[test]
fn malformed_file_is_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "hotkey = [not valid").unwrap();
    assert!(matches!(load(&path), Err(ConfigError::Parse(_))));
}

#[test]
fn partial_file_fills_missing_fields_with_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "sounds = false\n").unwrap();
    let config = load(&path).unwrap();
    assert!(!config.sounds);
    assert_eq!(
        config.max_recording_secs,
        Config::default().max_recording_secs
    );
    assert_eq!(config.hotkey, VirtualKey(0x78));
}
