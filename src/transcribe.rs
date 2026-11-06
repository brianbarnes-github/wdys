use std::path::{Path, PathBuf};

use crate::audio::AudioClip;
use crate::error::TranscribeError;
use crate::hotkey::Millis;

pub trait SpeechModel {
    fn transcribe(&mut self, clip: &AudioClip) -> Result<String, TranscribeError>;
}

pub trait ModelLoader {
    fn load(&self, path: &Path) -> Result<Box<dyn SpeechModel>, TranscribeError>;
}

/// Loads the model on first use and drops it again after a period of inactivity.
pub struct LazyModel {
    loader: Box<dyn ModelLoader>,
    model_path: PathBuf,
    idle_unload: Millis,
    loaded: Option<Box<dyn SpeechModel>>,
    last_used: Millis,
}

impl LazyModel {
    pub fn new(loader: Box<dyn ModelLoader>, model_path: PathBuf, idle_unload: Millis) -> LazyModel {
        LazyModel { loader, model_path, idle_unload, loaded: None, last_used: 0 }
    }

    pub fn transcribe(&mut self, clip: &AudioClip, now: Millis) -> Result<String, TranscribeError> {
        if self.loaded.is_none() {
            self.loaded = Some(self.loader.load(&self.model_path)?);
        }
        self.last_used = now;
        match self.loaded.as_mut() {
            Some(model) => model.transcribe(clip),
            None => Err(TranscribeError::Load("model unavailable".into())),
        }
    }

    /// Unloads the model if it has been idle for the configured time; true if it was unloaded.
    pub fn unload_if_idle(&mut self, now: Millis) -> bool {
        let idle = self.loaded.is_some() && now.saturating_sub(self.last_used) >= self.idle_unload;
        if idle {
            self.loaded = None;
        }
        idle
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded.is_some()
    }
}
