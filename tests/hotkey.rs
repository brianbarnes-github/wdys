use wdys::config::Config;
use wdys::hotkey::{step, Action, FsmConfig, Input, Millis, State};

const CFG: FsmConfig = FsmConfig { min_hold: 300, double_press_window: 400, max_recording: 120_000 };

/// Feeds events in order and returns the final state plus each event's actions.
fn run(events: &[(Input, Millis)]) -> (State, Vec<Vec<Action>>) {
    events.iter().fold((State::Idle, Vec::new()), |(state, mut log), &(input, now)| {
        let (next, actions) = step(&CFG, state, input, now);
        log.push(actions);
        (next, log)
    })
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
    let (_, log) = run(&[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300), (KeyUp, 400), (KeyDown, 5000)]);
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
    assert_eq!(log, vec![vec![StartRecording], vec![], vec![StopAndTranscribe]]);
}

#[test]
fn max_length_stops_hands_free() {
    let (state, log) = run(&[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300), (KeyUp, 400), (Tick, 120_300)]);
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
    let (_, log) = run(&[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300), (KeyDown, 330), (KeyDown, 360)]);
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
    let (state, log) = run(&[(KeyDown, 0), (Tick, 120_000), (KeyDown, 120_030), (KeyUp, 120_500)]);
    assert_eq!(log[2], vec![]);
    assert_eq!(state, State::Idle);
}

#[test]
fn auto_repeat_after_hands_free_stop_press_does_not_restart_recording() {
    let (state, log) =
        run(&[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300), (KeyUp, 400), (KeyDown, 5000), (KeyDown, 5030), (KeyUp, 5200)]);
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
    assert_eq!(fsm, FsmConfig { min_hold: 300, double_press_window: 400, max_recording: 120_000 });
}

#[test]
fn synthesizes_keyup_only_when_believed_down_but_actually_up() {
    use wdys::hotkey::should_synthesize_keyup;
    assert!(should_synthesize_keyup(true, false));
    assert!(!should_synthesize_keyup(true, true));
    assert!(!should_synthesize_keyup(false, false));
    assert!(!should_synthesize_keyup(false, true));
}
