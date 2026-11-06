use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("config could not be parsed: {0}")]
    Parse(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no input device available")]
    NoInputDevice,
    #[error("input device was lost")]
    DeviceLost,
    #[error("audio stream error: {0}")]
    Stream(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model download was declined")]
    DownloadDeclined,
    #[error("model checksum mismatch: {path}")]
    ChecksumMismatch { path: PathBuf },
    #[error("model download failed: {0}")]
    Download(String),
    #[error("model I/O error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum TranscribeError {
    #[error("could not load model: {0}")]
    Load(String),
    #[error("transcription failed: {0}")]
    Inference(String),
}

#[derive(Debug, thiserror::Error)]
pub enum PasteError {
    #[error("clipboard error: {0}")]
    Clipboard(String),
    #[error("keyboard input error: {0}")]
    Input(String),
    #[error("hotkey is still held")]
    HotkeyStillHeld,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Transcribe(#[from] TranscribeError),
    #[error(transparent)]
    Paste(#[from] PasteError),
}
