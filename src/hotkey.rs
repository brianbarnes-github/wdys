use crate::config::Config;

pub type Millis = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsmConfig {
    pub min_hold: Millis,
    pub double_press_window: Millis,
    pub max_recording: Millis,
}

impl From<&Config> for FsmConfig {
    fn from(config: &Config) -> Self {
        Self {
            min_hold: Millis::from(config.min_hold_ms),
            double_press_window: Millis::from(config.double_press_ms),
            max_recording: Millis::from(config.max_recording_secs) * 1000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    KeyDown,
    KeyUp,
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    StartRecording,
    CancelRecording,
    EnterHandsFree,
    StopAndTranscribe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Holding { since: Millis },
    AwaitingSecondTap { released_at: Millis },
    HandsFree { since: Millis, key_held: bool },
    WaitingRelease,
}

/// Pure transition function: given the current state and an input at time `now`,
/// returns the next state and the actions the caller must perform.
pub fn step(cfg: &FsmConfig, state: State, input: Input, now: Millis) -> (State, Vec<Action>) {
    use Action::*;
    use Input::*;
    use State::*;

    match (state, input) {
        (Idle, KeyDown) => (Holding { since: now }, vec![StartRecording]),
        (Idle, _) => (Idle, vec![]),

        (Holding { since }, KeyUp) if now - since >= cfg.min_hold => (Idle, vec![StopAndTranscribe]),
        (Holding { .. }, KeyUp) => (AwaitingSecondTap { released_at: now }, vec![CancelRecording]),
        (Holding { since }, Tick) if now - since >= cfg.max_recording => {
            (WaitingRelease, vec![StopAndTranscribe])
        }
        (Holding { since }, _) => (Holding { since }, vec![]),

        (AwaitingSecondTap { released_at }, KeyDown) if now - released_at < cfg.double_press_window => {
            (HandsFree { since: now, key_held: true }, vec![StartRecording, EnterHandsFree])
        }
        (AwaitingSecondTap { .. }, KeyDown) => (Holding { since: now }, vec![StartRecording]),
        (AwaitingSecondTap { released_at }, Tick) if now - released_at >= cfg.double_press_window => {
            (Idle, vec![])
        }
        (AwaitingSecondTap { released_at }, _) => (AwaitingSecondTap { released_at }, vec![]),

        (HandsFree { since, .. }, KeyUp) => (HandsFree { since, key_held: false }, vec![]),
        (HandsFree { key_held: false, .. }, KeyDown) => (WaitingRelease, vec![StopAndTranscribe]),
        (HandsFree { since, key_held }, Tick) if now - since >= cfg.max_recording => {
            (if key_held { WaitingRelease } else { Idle }, vec![StopAndTranscribe])
        }
        (HandsFree { since, key_held }, _) => (HandsFree { since, key_held }, vec![]),

        (WaitingRelease, KeyUp) => (Idle, vec![]),
        (WaitingRelease, _) => (WaitingRelease, vec![]),
    }
}
