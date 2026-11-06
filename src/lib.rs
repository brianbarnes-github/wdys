pub mod app;
pub mod audio;
pub mod config;
pub mod error;
pub mod filler;
pub mod hotkey;
pub mod icon;
pub mod keys;
pub mod model_store;
pub mod overlay;
pub mod paste;
#[cfg(windows)]
pub mod platform;
pub mod toast;
pub mod transcribe;
#[cfg(feature = "whisper")]
pub mod whisper;
