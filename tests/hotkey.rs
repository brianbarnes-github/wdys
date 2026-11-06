use wdys::config::Config;
use wdys::hotkey::{step, Action, FsmConfig, Input, Millis, State};

const CFG: FsmConfig = FsmConfig {
    min_hold: 300,
    double_press_window: 400,
    max_recording: 120_000,
};

/// Feeds events in order and returns the final state plus each event's actions.
fn run(events: &[(Input, Millis)]) -> (State, Vec<Vec<Action>>) {
    events.iter().fold(
        (State::Idle, Vec::new()),
        |(state, mut log), &(input, now)| {
            let (next, actions) = step(&CFG, state, input, now);
            log.push(actions);
            (next, log)
        },
    )
}

use Action::*;
use Input::*;

#[test]
fn hold_then_release_transcribes() {
    let (state, log) = run(&[(KeyDown, 0), (KeyUp, 1000)]);
    assert_eq!(log, vec![vec![StartRecording], vec![StopAndTranscribe]]);
    assert_eq!(state, State::Idle);
}

#[test]
fn short_tap_is_discarded() {
    let (_, log) = run(&[(KeyDown, 0), (KeyUp, 100)]);
    assert_eq!(log, vec![vec![StartRecording], vec![CancelRecording]]);
}

#[test]
fn double_press_enters_hands_free() {
    let (state, log) = run(&[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300)]);
    assert_eq!(log[2], vec![StartRecording, EnterHandsFree]);
    assert!(matches!(state, State::HandsFree { .. }));
}

#[test]
fn hands_free_stops_on_next_press() {
    let (_, log) = run(&[
        (KeyDown, 0),
        (KeyUp, 100),
        (KeyDown, 300),
        (KeyUp, 400),
        (KeyDown, 5000),
    ]);
    assert_eq!(log[4], vec![StopAndTranscribe]);
}

#[test]
fn tap_then_late_second_press_is_a_new_hold() {
    let (state, log) = run(&[(KeyDown, 0), (KeyUp, 100), (KeyDown, 600)]);
    assert_eq!(log[2], vec![StartRecording]);
    assert!(matches!(state, State::Holding { .. }));
}

#[test]
fn auto_repeat_down_is_ignored() {
    let (_, log) = run(&[(KeyDown, 0), (KeyDown, 30), (KeyDown, 60)]);
    assert_eq!(log, vec![vec![StartRecording], vec![], vec![]]);
}

#[test]
fn max_length_stops_hold_to_talk() {
    let (_, log) = run(&[(KeyDown, 0), (Tick, 119_999), (Tick, 120_000)]);
    assert_eq!(
        log,
        vec![vec![StartRecording], vec![], vec![StopAndTranscribe]]
    );
}

#[test]
fn max_length_stops_hands_free() {
    let (state, log) = run(&[
        (KeyDown, 0),
        (KeyUp, 100),
        (KeyDown, 300),
        (KeyUp, 400),
        (Tick, 120_300),
    ]);
    assert_eq!(log[4], vec![StopAndTranscribe]);
    assert_eq!(state, State::Idle);
}

#[test]
fn keyup_after_hands_free_entry_is_ignored() {
    let (state, log) = run(&[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300), (KeyUp, 500)]);
    assert_eq!(log[3], vec![]);
    assert!(matches!(state, State::HandsFree { .. }));
}

#[test]
fn auto_repeat_while_entering_hands_free_does_not_stop_it() {
    let (_, log) = run(&[
        (KeyDown, 0),
        (KeyUp, 100),
        (KeyDown, 300),
        (KeyDown, 330),
        (KeyDown, 360),
    ]);
    assert_eq!(log[3], vec![]);
    assert_eq!(log[4], vec![]);
}

#[test]
fn stray_keyup_in_idle_is_ignored() {
    let (state, log) = run(&[(KeyUp, 50)]);
    assert_eq!(log, vec![vec![]]);
    assert_eq!(state, State::Idle);
}

#[test]
fn auto_repeat_after_forced_stop_does_not_restart_recording() {
    let (state, log) = run(&[
        (KeyDown, 0),
        (Tick, 120_000),
        (KeyDown, 120_030),
        (KeyUp, 120_500),
    ]);
    assert_eq!(log[2], vec![]);
    assert_eq!(state, State::Idle);
}

#[test]
fn auto_repeat_after_hands_free_stop_press_does_not_restart_recording() {
    let (state, log) = run(&[
        (KeyDown, 0),
        (KeyUp, 100),
        (KeyDown, 300),
        (KeyUp, 400),
        (KeyDown, 5000),
        (KeyDown, 5030),
        (KeyUp, 5200),
    ]);
    assert_eq!(log[5], vec![]);
    assert_eq!(state, State::Idle);
}

#[test]
fn unused_second_tap_window_expires() {
    let (state, _) = run(&[(KeyDown, 0), (KeyUp, 100), (Tick, 501)]);
    assert_eq!(state, State::Idle);
}

#[test]
fn fsm_config_is_derived_from_app_config() {
    let fsm = FsmConfig::from(&Config::default());
    assert_eq!(
        fsm,
        FsmConfig {
            min_hold: 300,
            double_press_window: 400,
            max_recording: 120_000
        }
    );
}

mod hook_decisions {
    use wdys::hotkey::{decide, Decision, HookState, Input, KeyEvent};

    const HOTKEY: u32 = 0x78;
    const OTHER: u32 = 0x41;

    fn state(held: bool, capturing: bool) -> HookState {
        HookState {
            hotkey: HOTKEY,
            held,
            capturing,
        }
    }

    fn key(vk: u32, is_down: bool) -> KeyEvent {
        KeyEvent {
            vk,
            is_down,
            injected: false,
        }
    }

    #[test]
    fn hotkey_press_is_swallowed_and_starts_the_sequence() {
        let d = decide(state(false, false), key(HOTKEY, true));
        assert_eq!(
            d,
            Decision {
                swallow: true,
                input: Some(Input::KeyDown),
                captured: None,
                held: true
            }
        );
    }

    #[test]
    fn auto_repeat_is_swallowed_without_a_second_keydown() {
        let d = decide(state(true, false), key(HOTKEY, true));
        assert_eq!(
            d,
            Decision {
                swallow: true,
                input: None,
                captured: None,
                held: true
            }
        );
    }

    #[test]
    fn hotkey_release_is_swallowed_and_ends_the_sequence() {
        let d = decide(state(true, false), key(HOTKEY, false));
        assert_eq!(
            d,
            Decision {
                swallow: true,
                input: Some(Input::KeyUp),
                captured: None,
                held: false
            }
        );
    }

    #[test]
    fn stray_hotkey_release_is_swallowed_but_ignored() {
        let d = decide(state(false, false), key(HOTKEY, false));
        assert_eq!(
            d,
            Decision {
                swallow: true,
                input: None,
                captured: None,
                held: false
            }
        );
    }

    #[test]
    fn other_keys_pass_through_untouched() {
        for down in [true, false] {
            let d = decide(state(false, false), key(OTHER, down));
            assert_eq!(
                d,
                Decision {
                    swallow: false,
                    input: None,
                    captured: None,
                    held: false
                }
            );
        }
    }

    #[test]
    fn other_keys_pass_through_while_the_hotkey_is_held() {
        let d = decide(state(true, false), key(OTHER, true));
        assert_eq!((d.swallow, d.input, d.held), (false, None, true));
    }

    #[test]
    fn injected_events_are_never_blocked_or_acted_on() {
        // Our own simulated Ctrl+V must reach the app, and must not feed back into the hook.
        let injected = KeyEvent {
            vk: HOTKEY,
            is_down: true,
            injected: true,
        };
        let d = decide(state(false, false), injected);
        assert_eq!(
            d,
            Decision {
                swallow: false,
                input: None,
                captured: None,
                held: false
            }
        );
    }

    #[test]
    fn capturing_takes_the_next_key_press_and_swallows_it() {
        let d = decide(state(false, true), key(OTHER, true));
        assert_eq!(
            d,
            Decision {
                swallow: true,
                input: None,
                captured: Some(OTHER),
                held: false
            }
        );
    }

    #[test]
    fn capturing_ignores_key_releases() {
        let d = decide(state(false, true), key(OTHER, false));
        assert_eq!(d.captured, None);
        assert!(!d.swallow);
    }
}
