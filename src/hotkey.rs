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

        (Holding { since }, KeyUp) if now.saturating_sub(since) >= cfg.min_hold => {
            (Idle, vec![StopAndTranscribe])
        }
        (Holding { .. }, KeyUp) => (
            AwaitingSecondTap { released_at: now },
            vec![CancelRecording],
        ),
        (Holding { since }, Tick) if now.saturating_sub(since) >= cfg.max_recording => {
            (WaitingRelease, vec![StopAndTranscribe])
        }
        (Holding { since }, _) => (Holding { since }, vec![]),

        (AwaitingSecondTap { released_at }, KeyDown)
            if now.saturating_sub(released_at) < cfg.double_press_window =>
        {
            (
                HandsFree {
                    since: now,
                    key_held: true,
                },
                vec![StartRecording, EnterHandsFree],
            )
        }
        (AwaitingSecondTap { .. }, KeyDown) => (Holding { since: now }, vec![StartRecording]),
        (AwaitingSecondTap { released_at }, Tick)
            if now.saturating_sub(released_at) >= cfg.double_press_window =>
        {
            (Idle, vec![])
        }
        (AwaitingSecondTap { released_at }, _) => (AwaitingSecondTap { released_at }, vec![]),

        (HandsFree { since, .. }, KeyUp) => (
            HandsFree {
                since,
                key_held: false,
            },
            vec![],
        ),
        (
            HandsFree {
                key_held: false, ..
            },
            KeyDown,
        ) => (WaitingRelease, vec![StopAndTranscribe]),
        (HandsFree { since, key_held }, Tick) if now.saturating_sub(since) >= cfg.max_recording => {
            (
                if key_held { WaitingRelease } else { Idle },
                vec![StopAndTranscribe],
            )
        }
        (HandsFree { since, key_held }, _) => (HandsFree { since, key_held }, vec![]),

        (WaitingRelease, KeyUp) => (Idle, vec![]),
        (WaitingRelease, _) => (WaitingRelease, vec![]),
    }
}

/// A keyboard event as seen by the low-level hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub vk: u32,
    pub is_down: bool,
    /// Synthesized by software (e.g. our own Ctrl+V); never ours to act on or block.
    pub injected: bool,
}

/// What the hook knows when an event arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookState {
    pub hotkey: u32,
    /// The hotkey is currently held, according to the events we have seen.
    pub held: bool,
    /// The tray's "Set hotkey" is waiting for the next key press.
    pub capturing: bool,
}

/// What the hook should do with an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// Keep the event from reaching the focused app.
    pub swallow: bool,
    /// Input to feed the state machine, if any.
    pub input: Option<Input>,
    /// The key chosen while capturing a new hotkey.
    pub captured: Option<u32>,
    /// The new "hotkey held" state.
    pub held: bool,
}

/// Decides what to do with a key event. The hotkey is always swallowed (so it never types or
/// triggers shortcuts in the app being dictated into), auto-repeat is collapsed to one
/// `KeyDown`, and injected events are left alone.
pub fn decide(state: HookState, event: KeyEvent) -> Decision {
    let pass = Decision {
        swallow: false,
        input: None,
        captured: None,
        held: state.held,
    };
    if event.injected {
        return pass;
    }
    if state.capturing && event.is_down {
        return Decision {
            swallow: true,
            captured: Some(event.vk),
            ..pass
        };
    }
    if event.vk != state.hotkey {
        return pass;
    }
    match (state.held, event.is_down) {
        (false, true) => Decision {
            swallow: true,
            input: Some(Input::KeyDown),
            held: true,
            ..pass
        },
        (true, false) => Decision {
            swallow: true,
            input: Some(Input::KeyUp),
            held: false,
            ..pass
        },
        _ => Decision {
            swallow: true,
            ..pass
        },
    }
}
