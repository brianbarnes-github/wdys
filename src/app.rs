use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::audio::{is_effectively_silent, Recorder};
use crate::config::Config;
use crate::error::AppError;
use crate::filler::clean_transcript;
use crate::hotkey::{step, Action, FsmConfig, Input, Millis, State};
use crate::paste::{paste_text, Clipboard, Keyboard};
use crate::transcribe::LazyModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Idle,
    Recording,
    HandsFree,
    Transcribing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Start,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    Tray(TrayState),
    Cue(Cue),
    Notify(String),
}

/// Wires the hotkey state machine to recording, transcription, cleanup and paste.
pub struct App<R: Recorder, C: Clipboard, K: Keyboard> {
    config: Config,
    fsm: FsmConfig,
    state: State,
    recorder: R,
    model: LazyModel,
    clipboard: C,
    keyboard: K,
    sleep: Box<dyn Fn(Duration)>,
}

impl<R: Recorder, C: Clipboard, K: Keyboard> App<R, C, K> {
    pub fn new(
        config: Config,
        recorder: R,
        model: LazyModel,
        clipboard: C,
        keyboard: K,
        sleep: Box<dyn Fn(Duration)>,
    ) -> Self {
        let fsm = FsmConfig::from(&config);
        Self { config, fsm, state: State::Idle, recorder, model, clipboard, keyboard, sleep }
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// Forgets any in-flight key sequence; used after input was dropped during transcription.
    pub fn reset_input(&mut self) {
        self.state = State::Idle;
    }

    /// Processes one input, reporting UI changes through `emit` as they happen.
    pub fn handle(&mut self, input: Input, now: Millis, emit: &mut dyn FnMut(UiEvent)) {
        if input == Input::Tick {
            self.model.unload_if_idle(now);
        }
        let (next, actions) = step(&self.fsm, self.state, input, now);
        self.state = next;
        for action in actions {
            if !self.perform(action, now, emit) {
                break;
            }
        }
    }

    fn cue(&self, cue: Cue, emit: &mut dyn FnMut(UiEvent)) {
        if self.config.sounds {
            emit(UiEvent::Cue(cue));
        }
    }

    /// Returns false when the action failed and the remaining actions must be skipped.
    fn perform(&mut self, action: Action, now: Millis, emit: &mut dyn FnMut(UiEvent)) -> bool {
        match action {
            Action::StartRecording => match self.recorder.start() {
                Ok(()) => {
                    self.cue(Cue::Start, emit);
                    emit(UiEvent::Tray(TrayState::Recording));
                    true
                }
                Err(e) => {
                    self.state = State::Idle;
                    emit(UiEvent::Notify(e.to_string()));
                    emit(UiEvent::Tray(TrayState::Idle));
                    false
                }
            },
            Action::EnterHandsFree => {
                emit(UiEvent::Tray(TrayState::HandsFree));
                true
            }
            Action::CancelRecording => {
                let _ = self.recorder.stop();
                emit(UiEvent::Tray(TrayState::Idle));
                true
            }
            Action::StopAndTranscribe => {
                self.cue(Cue::Stop, emit);
                emit(UiEvent::Tray(TrayState::Transcribing));
                if let Err(e) = self.finish_recording(now) {
                    emit(UiEvent::Notify(e.to_string()));
                }
                emit(UiEvent::Tray(TrayState::Idle));
                true
            }
        }
    }

    fn finish_recording(&mut self, now: Millis) -> Result<(), AppError> {
        let clip = self.recorder.stop()?;
        if is_effectively_silent(&clip) {
            return Ok(());
        }
        let raw = self.model.transcribe(&clip, now)?;
        let text = clean_transcript(&raw);
        paste_text(&mut self.clipboard, &mut self.keyboard, self.config.hotkey, &text, &*self.sleep)?;
        Ok(())
    }
}

/// Runs inputs in order. Anything that queued up while a transcription was running is
/// dropped, so a stale key press can never start a recording after the fact.
pub fn run_worker<R: Recorder, C: Clipboard, K: Keyboard>(
    rx: Receiver<(Input, Millis)>,
    app: &mut App<R, C, K>,
    on_event: &dyn Fn(UiEvent),
) {
    while let Ok((input, now)) = rx.recv() {
        let mut transcribed = false;
        app.handle(input, now, &mut |event| {
            transcribed |= event == UiEvent::Tray(TrayState::Transcribing);
            on_event(event);
        });
        if transcribed {
            while rx.try_recv().is_ok() {}
            app.reset_input();
        }
    }
}
