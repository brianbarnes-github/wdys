//! whisper.cpp-backed implementation of [`SpeechModel`], compiled with the `whisper` feature.

use std::path::Path;
use std::thread;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio::AudioClip;
use crate::error::TranscribeError;
use crate::transcribe::{ModelLoader, SpeechModel};

struct WhisperModel {
    context: WhisperContext,
    threads: i32,
}

impl SpeechModel for WhisperModel {
    fn transcribe(&mut self, clip: &AudioClip) -> Result<String, TranscribeError> {
        let inference = |e: whisper_rs::WhisperError| TranscribeError::Inference(e.to_string());
        let mut state = self.context.create_state().map_err(inference)?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_n_threads(self.threads);
        params.set_translate(false);
        params.set_no_timestamps(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        state.full(params, &clip.samples).map_err(inference)?;
        Ok(state
            .as_iter()
            .map(|segment| segment.to_string())
            .collect::<String>()
            .trim()
            .to_string())
    }
}

/// Loads English Whisper models via whisper.cpp, using all cores but one.
pub struct WhisperLoader;

impl ModelLoader for WhisperLoader {
    fn load(&self, path: &Path) -> Result<Box<dyn SpeechModel>, TranscribeError> {
        let path = path
            .to_str()
            .ok_or_else(|| TranscribeError::Load("model path is not valid UTF-8".into()))?;
        let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| TranscribeError::Load(e.to_string()))?;
        let cores = thread::available_parallelism().map_or(1, |n| n.get());
        let threads = i32::try_from(cores.saturating_sub(1).max(1)).unwrap_or(1);
        Ok(Box::new(WhisperModel { context, threads }))
    }
}
