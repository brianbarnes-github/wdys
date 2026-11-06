pub mod config;
pub mod error;
pub mod filler;
pub mod hotkey;
pub mod audio;
pub mod paste;
pub mod model_store;
pub mod transcribe;
#[cfg(feature = "whisper")]
pub mod whisper;
pub mod app;
#[cfg(windows)]
pub mod platform;
pub mod icon;
pub mod keys;
pub mod toast;
pub mod overlay;
