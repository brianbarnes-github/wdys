# Voice Dictation Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A lightweight, local-only Windows tray app: hold a user-defined key to record, release to transcribe with Whisper, strip filler words, and paste into the focused app; double-press for hands-free.

**Architecture:** A Rust library crate holds all logic as pure functions and small traits (`Recorder`, `SpeechModel`, `Clipboard`, `Keyboard`, `Downloader`, `Consent`) so everything is tested on Linux/WSL with fakes. A thin `#[cfg(windows)]` layer implements those traits with Win32, `cpal` and `tray-icon`, and feeds key events to one worker thread that drives a pure hotkey state machine.

**Tech Stack:** Rust (stable), `whisper-rs`, `cpal`, `tray-icon`, `windows`, `thiserror`, `serde` + `toml`, `sha2`, `ureq`.

**Spec:** `docs/superpowers/specs/2026-11-06-voice-dictation-tool-design.md`

## Global Constraints

- Windows target; development and logic tests run on Linux/WSL. Windows-only code lives under `src/platform/` behind `#[cfg(windows)]`.
- Local only. The single permitted network use is the first-run model download, after the user consents, checksum-verified. Audio and text never leave the machine.
- English only; model `base.en` by default, `small.en` selectable; CPU only.
- Lightweight: no Python, no Electron; model loads on first use and unloads after idle (default 600 s).
- Defaults: min hold (tap threshold) 300 ms; double-press window 400 ms; max recording 120 s for both modes; hotkey default Right Ctrl (VK `0xA3`), user-changeable.
- The hotkey is NOT blocked: the low-level hook always calls the next hook.
- Paste is Ctrl+V after the hotkey is released; clipboard restored afterward (text only); dictated text excluded from clipboard history.
- Process rules (user's): test-first; integration tests preferred for business logic; functional style over classes; full types; no stringly errors, use `thiserror` custom error types; conventional commits, each ending with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Dependencies are new and were disclosed to the user in the design; mention each again in the commit that adds it.

## Review Focus

Input classes the spec implies but nothing else pins, most likely first. Each has a test in the task named.

1. Words that contain a filler (`umbrella`, `human`, `Ahmed`, `error`) must survive untouched. (Task 2)
2. Silent or filler-only recordings, or Whisper markers like `[BLANK_AUDIO]`, must paste nothing and leave the clipboard alone. (Tasks 2, 5, 8)
3. Hotkey pressed again while a transcription is running must not start a second recording. (Task 8)
4. Empty or non-text clipboard when pasting must not leave dictated text behind afterward. (Task 5)
5. Model missing, download declined, or checksum mismatch must produce a clear notification, never a crash or a half-written model file. (Task 6)

## File Structure

```
Cargo.toml                  features: default=["whisper"]; whisper = ["dep:whisper-rs"]
src/lib.rs                  module declarations
src/error.rs                custom error types (thiserror)
src/config.rs               Config, load/save TOML
src/filler.rs               clean_transcript
src/hotkey.rs               pure state machine
src/audio.rs                to_whisper_input, AudioClip, Recorder trait
src/paste.rs                Clipboard/Keyboard traits, paste_text
src/model_store.rs          model paths, checksum, ensure_model
src/transcribe.rs           SpeechModel/ModelLoader traits, LazyModel
src/app.rs                  App (wires everything), run_worker
src/platform/mod.rs         cfg(windows) glue
src/platform/{hook,clipboard,keyboard,recorder,tray,autostart,instance,cues}.rs
src/main.rs                 windows entry point
tests/                      integration tests (one file per task 3..8)
```

---

### Task 1: Scaffold, errors, config

**Files:**
- Create: `Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/config.rs`, `tests/config.rs`

**Interfaces:**
- Produces:
  - `pub struct VirtualKey(pub u32)` (`Copy, Eq, Serialize, Deserialize`)
  - `pub enum ModelSize { BaseEn, SmallEn }`
  - `pub struct Config { hotkey: VirtualKey, model: ModelSize, max_recording_secs: u32, idle_unload_secs: u32, double_press_ms: u32, min_hold_ms: u32, sounds: bool, start_with_windows: bool }` with `Default` using the Global Constraints values
  - `pub fn load(path: &Path) -> Result<Config, ConfigError>` (missing file returns default; malformed returns `ConfigError::Parse`)
  - `pub fn save(path: &Path, config: &Config) -> Result<(), ConfigError>`
  - `pub enum ConfigError { Io(#[from] io::Error), Parse(String) }`; stubs for `AudioError`, `ModelError`, `TranscribeError`, `PasteError` in `error.rs`, and `pub enum AppError` wrapping all five with `#[from]`.

- [ ] **Step 1: Write failing tests** in `tests/config.rs`: `missing_file_yields_defaults` (`load(tmp/"nope.toml") == Ok(Config::default())`, and `default().hotkey == VirtualKey(0xA3)`); `save_then_load_round_trips` (change `hotkey` to `VirtualKey(0x7B)` and `model` to `SmallEn`, equal after reload); `malformed_file_is_parse_error` (`matches!(load(..), Err(ConfigError::Parse(_)))`); `partial_file_fills_missing_fields_with_defaults`.
- [ ] **Step 2:** `cargo init --lib`, add dependencies `thiserror`, `serde` (derive), `toml`, dev-dependency `tempfile`; make tests compile with `todo!()` bodies; run `cargo test --no-default-features` and confirm FAIL.
- [ ] **Step 3: Implement** the interfaces above in `src/config.rs` and `src/error.rs` (`#[serde(default)]` on `Config`).
- [ ] **Step 4:** `cargo test --no-default-features` passes; `cargo clippy --no-default-features -- -D warnings` clean.
- [ ] **Step 5: Commit** `feat: scaffold crate with config and error types`.

### Task 2: Filler removal

**Files:** Create `src/filler.rs`, `tests/filler.rs`

**Interfaces:**
- Produces: `pub fn clean_transcript(raw: &str) -> String` (returns `""` when nothing meaningful remains)

Rules: remove standalone, case-insensitive `um, umm, uh, uhh, ah, ahh, er, erm, hm, hmm` (not `you know`, `like`, `so`, which carry meaning); collapse an immediate repeat of the same word of 3 letters or fewer (`I I I want` becomes `I want`); strip Whisper markers (`[BLANK_AUDIO]`, `[ Silence ]`, `(music)`, text wrapped in `[]` or `()` that is a whole token run); tidy orphaned commas and double spaces; re-capitalize the first letter if a leading filler was removed.

- [ ] **Step 1: Write failing tests:**
  - `removes_basic_fillers`: `"So, um, I think uh we should go"` becomes `"So, I think we should go"`
  - `leading_filler_recapitalizes`: `"Um, hello there"` becomes `"Hello there"`
  - `keeps_words_containing_fillers` (Review Focus 1): `"The umbrella is human, Ahmed made an error"` is unchanged
  - `collapses_short_stutters`: `"I I I want the the thing"` becomes `"I want the thing"`; `"that that"` becomes `"that"` (accepted tradeoff, noted in the test comment)
  - `strips_whisper_markers` (Review Focus 2): `"[BLANK_AUDIO]"` becomes `""`; `"hello [ Silence ]"` becomes `"hello"`
  - `filler_only_becomes_empty`: `"Um. Uh, ahh."` becomes `""`
- [ ] **Step 2:** `cargo test --no-default-features --test filler` fails.
- [ ] **Step 3: Implement** `clean_transcript` as a pipeline of small pure functions (tokenize, drop, collapse, rejoin); no regex dependency unless it clearly simplifies, and mention it if added.
- [ ] **Step 4:** tests pass.
- [ ] **Step 5: Commit** `feat: add filler word removal`.

### Task 3: Hotkey state machine

**Files:** Create `src/hotkey.rs`, `tests/hotkey.rs`

**Interfaces:**
- Produces:
  - `pub type Millis = u64`
  - `pub struct FsmConfig { pub min_hold: Millis, pub double_press_window: Millis, pub max_recording: Millis }` plus `impl From<&Config> for FsmConfig`
  - `pub enum Input { KeyDown, KeyUp, Tick }`
  - `pub enum Action { StartRecording, CancelRecording, EnterHandsFree, StopAndTranscribe }`
  - `pub enum State { Idle, Holding { since: Millis }, AwaitingSecondTap { released_at: Millis }, HandsFree { since: Millis } }`
  - `pub fn step(cfg: &FsmConfig, state: State, input: Input, now: Millis) -> (State, Vec<Action>)`

Behavior: `Idle` + `KeyDown` starts recording immediately (`Holding`). `KeyDown` while `Holding`/`HandsFree`-held is auto-repeat and ignored, except in `HandsFree` where a press after the entering press stops. `KeyUp` after at least `min_hold` stops and transcribes (`Idle`); shorter cancels and moves to `AwaitingSecondTap`. A `KeyDown` within `double_press_window` of that release starts recording and enters `HandsFree`; `Tick` after the window expires returns to `Idle`. In `Holding` and `HandsFree`, `Tick` at `since + max_recording` or later stops and transcribes. The key-up that follows hands-free entry is ignored.

- [ ] **Step 1: Write failing tests** (cfg: min_hold 300, window 400, max 120_000): `hold_then_release_transcribes` (Down@0, Up@1000 yields `[StartRecording]` then `[StopAndTranscribe]`, ends `Idle`); `short_tap_is_discarded` (Up@100 yields `[CancelRecording]`); `double_press_enters_hands_free` (tap, then Down@300 yields `[StartRecording, EnterHandsFree]`); `hands_free_stops_on_next_press` (Down@5000 yields `[StopAndTranscribe]`); `tap_then_late_second_press_is_a_new_hold` (second Down@600 is plain `StartRecording`); `auto_repeat_down_is_ignored` (Down@0, Down@30, Down@60 emits one `StartRecording`); `max_length_stops_hold_to_talk` and `max_length_stops_hands_free` (Tick@120_000 yields `StopAndTranscribe`); `keyup_after_hands_free_entry_is_ignored`; `stray_keyup_in_idle_is_ignored`.
- [ ] **Step 2:** run, expect FAIL.
- [ ] **Step 3: Implement** `step` as a single pure `match (state, input)`.
- [ ] **Step 4:** tests pass.
- [ ] **Step 5: Commit** `feat: add hotkey state machine`.

### Task 4: Audio conversion

**Files:** Create `src/audio.rs`, `tests/audio.rs`

**Interfaces:**
- Produces:
  - `pub struct AudioClip { pub samples: Vec<f32> }` (16 kHz mono)
  - `pub fn to_whisper_input(samples: &[f32], sample_rate: u32, channels: u16) -> AudioClip` (downmix to mono by averaging, linear resample to 16 000 Hz)
  - `pub fn is_effectively_silent(clip: &AudioClip) -> bool` (RMS below `0.003`, or shorter than 0.3 s)
  - `pub trait Recorder { fn start(&mut self) -> Result<(), AudioError>; fn stop(&mut self) -> Result<AudioClip, AudioError>; }`
  - `AudioError` variants: `NoInputDevice`, `DeviceLost`, `Stream(String)`

- [ ] **Step 1: Write failing tests:** `stereo_48k_becomes_mono_16k` (96 000 interleaved samples at 48 kHz stereo yields 16 000 samples); `already_16k_mono_is_unchanged`; `downmix_averages_channels` (`[1.0, -1.0]` stereo yields `[0.0]`); `silent_clip_is_detected` and `one_second_of_tone_is_not_silent`; `very_short_clip_counts_as_silent`.
- [ ] **Step 2:** run, expect FAIL. **Step 3:** implement. **Step 4:** pass.
- [ ] **Step 5: Commit** `feat: add audio conversion and silence detection`.

### Task 5: Paste flow

**Files:** Create `src/paste.rs`, `tests/paste.rs`

**Interfaces:**
- Produces:
  - `pub trait Clipboard { fn get_text(&mut self) -> Result<Option<String>, PasteError>; fn set_text(&mut self, text: &str, exclude_from_history: bool) -> Result<(), PasteError>; fn clear(&mut self) -> Result<(), PasteError>; }`
  - `pub trait Keyboard { fn send_paste(&mut self) -> Result<(), PasteError>; fn is_down(&self, key: VirtualKey) -> bool; }`
  - `pub fn paste_text(clipboard: &mut impl Clipboard, keyboard: &mut impl Keyboard, hotkey: VirtualKey, text: &str, sleep: &dyn Fn(Duration)) -> Result<(), PasteError>`
  - `PasteError` variants: `Clipboard(String)`, `Input(String)`, `HotkeyStillHeld`

Behavior: empty `text` does nothing at all. Otherwise wait (polling via `sleep`, 10 ms steps, up to 1 s) until `hotkey` is up, else `HotkeyStillHeld`; save previous text; `set_text(text, true)`; `send_paste`; `sleep(150 ms)`; restore previous text, or `clear()` if there was none.

- [ ] **Step 1: Write failing tests** with in-memory fakes: `pastes_and_restores_previous_text` (final clipboard equals the prior text, one paste sent, dictated text marked excluded); `empty_text_touches_nothing` (Review Focus 2: no clipboard writes, no paste); `empty_clipboard_is_cleared_afterward` (Review Focus 4: after paste, `get_text() == None`); `waits_for_hotkey_release` (fake keyboard reports down for 3 polls, then up; paste happens after); `held_hotkey_times_out` (always down gives `Err(HotkeyStillHeld)` and nothing was pasted); `restores_even_when_paste_fails` (send_paste errors, clipboard still restored).
- [ ] **Step 2:** run, expect FAIL. **Step 3:** implement. **Step 4:** pass.
- [ ] **Step 5: Commit** `feat: add clipboard paste flow`.

### Task 6: Model store

**Files:** Create `src/model_store.rs`, `tests/model_store.rs`

**Interfaces:**
- Produces:
  - `pub fn model_path(dir: &Path, size: ModelSize) -> PathBuf` (`ggml-base.en.bin` / `ggml-small.en.bin`)
  - `pub fn model_url(size: ModelSize) -> &'static str`, `pub fn model_sha256(size: ModelSize) -> &'static str`; the builder pins these from the published ggerganov/whisper.cpp Hugging Face files and does not invent them
  - `pub fn verify_checksum(path: &Path, expected_hex: &str) -> Result<(), ModelError>`
  - `pub trait Downloader { fn fetch(&self, url: &str, dest: &Path) -> Result<(), ModelError>; }`
  - `pub trait Consent { fn allow_download(&self, size: ModelSize, approx_bytes: u64) -> bool; }`
  - `pub fn ensure_model(dir: &Path, size: ModelSize, downloader: &dyn Downloader, consent: &dyn Consent, expected_sha256: &str) -> Result<PathBuf, ModelError>`
  - `ModelError` variants: `DownloadDeclined`, `ChecksumMismatch { path: PathBuf }`, `Download(String)`, `Io(#[from] io::Error)`

Behavior: existing file with a good checksum is returned with no consent prompt. Otherwise ask consent, download to `<name>.part`, verify, then rename into place. A bad download deletes the `.part` file. A bad existing file is treated as missing (re-offer download). Manual placement works because a correct file already in `dir` satisfies the first rule.

- [ ] **Step 1: Write failing tests** with fake downloader/consent (use a tiny fixture with a known sha256): `existing_valid_model_needs_no_download_or_prompt`; `missing_model_downloads_after_consent` (file ends at final path, no `.part` left); `declined_consent_is_error_and_downloads_nothing` (Review Focus 5); `checksum_mismatch_deletes_part_file_and_errors` (Review Focus 5; final path absent); `corrupt_existing_model_is_redownloaded`; `downloader_failure_leaves_no_files`.
- [ ] **Step 2:** run, expect FAIL. **Step 3:** implement (new deps `sha2`, `ureq`; the `ureq` `Downloader` impl is a thin struct and is not unit-tested). **Step 4:** pass.
- [ ] **Step 5: Commit** `feat: add model download and verification`.

### Task 7: Transcriber with idle unload

**Files:** Create `src/transcribe.rs`, `tests/transcribe.rs`; the real `whisper-rs` loader behind `#[cfg(feature = "whisper")]`

**Interfaces:**
- Consumes: `AudioClip`, `Millis`
- Produces:
  - `pub trait SpeechModel { fn transcribe(&mut self, clip: &AudioClip) -> Result<String, TranscribeError>; }`
  - `pub trait ModelLoader { fn load(&self, path: &Path) -> Result<Box<dyn SpeechModel>, TranscribeError>; }`
  - `pub struct LazyModel` with `pub fn new(loader: Box<dyn ModelLoader>, model_path: PathBuf, idle_unload: Millis) -> LazyModel`, `pub fn transcribe(&mut self, clip: &AudioClip, now: Millis) -> Result<String, TranscribeError>`, `pub fn unload_if_idle(&mut self, now: Millis) -> bool`, `pub fn is_loaded(&self) -> bool`
  - `pub struct WhisperLoader` (feature `whisper`) implementing `ModelLoader`: English, greedy decoding, no timestamps, thread count = available cores minus one (min 1)
  - `TranscribeError` variants: `Load(String)`, `Inference(String)`

- [ ] **Step 1: Write failing tests** with a counting fake loader: `loads_on_first_use_only` (two transcribes, one load); `unloads_after_idle_timeout` (`unload_if_idle(now = last + idle)` returns true, `is_loaded()` false); `does_not_unload_before_timeout`; `reloads_after_unload`; `load_failure_surfaces_as_error_and_retries_next_time`.
- [ ] **Step 2:** run, expect FAIL. **Step 3:** implement; the `WhisperLoader` is compiled only with the `whisper` feature and verified in Task 11 on Windows. **Step 4:** pass.
- [ ] **Step 5: Commit** `feat: add lazy speech model with idle unload`.

### Task 8: App pipeline (integration)

**Files:** Create `src/app.rs`, `tests/app.rs`

**Interfaces:**
- Consumes: everything from Tasks 1-7.
- Produces:
  - `pub enum UiEvent { Tray(TrayState), Cue(Cue), Notify(String) }`, `pub enum TrayState { Idle, Recording, HandsFree, Transcribing }`, `pub enum Cue { Start, Stop }`
  - `pub struct App<R: Recorder, C: Clipboard, K: Keyboard>` constructed by `App::new(config: Config, recorder: R, model: LazyModel, clipboard: C, keyboard: K, sleep: Box<dyn Fn(Duration)>) -> App<R, C, K>`
  - `pub fn handle(&mut self, input: Input, now: Millis) -> Vec<UiEvent>` (runs the FSM; on `StopAndTranscribe` runs record-stop, silence check, transcribe, `clean_transcript`, `paste_text`; any `AppError` becomes `UiEvent::Notify` and the app returns to `Idle`)
  - `pub fn run_worker<R: Recorder, C: Clipboard, K: Keyboard>(rx: Receiver<(Input, Millis)>, app: &mut App<R, C, K>, on_event: &dyn Fn(UiEvent))`: processes inputs in order; after a transcription completes it drains and drops everything queued meanwhile and resets the FSM to `Idle`

- [ ] **Step 1: Write failing integration tests** (fakes for all traits, whole pipeline):
  - `hold_speak_release_pastes_cleaned_text`: fake model returns `"Um, hello world"`, clipboard ends with the prior text and exactly `"Hello world"` was pasted
  - `double_press_hands_free_pastes_on_next_press`
  - `short_tap_pastes_nothing_and_cancels_recording`
  - `silent_clip_pastes_nothing` and `filler_only_transcript_pastes_nothing` (Review Focus 2; the model is not even called for the silent one)
  - `max_length_tick_pastes`
  - `input_during_transcription_is_dropped` (Review Focus 3): the fake model enqueues `KeyDown, KeyUp` into the worker channel while transcribing; after it finishes, no second recording is started and the state is `Idle`
  - `recorder_failure_notifies_and_recovers` (`DeviceLost` yields a `Notify`, the next hold works)
  - `tray_states_progress_idle_recording_transcribing_idle`
- [ ] **Step 2:** run, expect FAIL. **Step 3:** implement; keep `handle` thin by composing the existing pure functions. **Step 4:** pass, plus `cargo test --no-default-features` for the whole crate.
- [ ] **Step 5: Commit** `feat: add app pipeline`.

### Task 9: Windows input (hook, tick, hotkey capture)

**Files:** Create `src/platform/mod.rs`, `src/platform/hook.rs`

**Interfaces:**
- Consumes: `Input`, `Millis`, `VirtualKey`
- Produces:
  - `pub fn install_hook(hotkey: Arc<AtomicU32>, tx: Sender<(Input, Millis)>) -> Result<HookHandle, HookError>`; a `WH_KEYBOARD_LL` hook that sends `KeyDown`/`KeyUp` for the hotkey only, always calls `CallNextHookEx` (never blocks the key), and ignores events with `LLKHF_INJECTED` so our own simulated Ctrl+V never loops back
  - `pub fn spawn_ticker(tx, interval_ms: u64 = 200)`: sends `Tick`, and also synthesizes `KeyUp` if `GetAsyncKeyState(hotkey)` shows the key up while the state machine believes it is held (lost key-up safeguard)
  - `pub fn capture_next_key() -> Result<VirtualKey, HookError>` for the tray's "Set hotkey"
  - `HookError` in `error.rs`

The pure parts of the lost-key-up logic are a function `pub fn should_synthesize_keyup(believed_down: bool, actually_down: bool) -> bool`, unit-tested on Linux (`believed true, actual false` yields true, other three cases false).

- [ ] **Step 1:** add the failing test for `should_synthesize_keyup` in `tests/hotkey.rs`; run, expect FAIL.
- [ ] **Step 2:** implement it, then the Windows hook code; confirm Linux tests pass and `cargo check --no-default-features --target x86_64-pc-windows-gnu` compiles the platform layer.
- [ ] **Step 3: Commit** `feat: add windows keyboard hook and ticker`.

### Task 10: Windows output (clipboard, keyboard, recorder, cues)

**Files:** Create `src/platform/{clipboard,keyboard,recorder,cues}.rs`

**Interfaces:**
- Produces: `WinClipboard: Clipboard` (`CF_UNICODETEXT`; `set_text(.., true)` also registers `ExcludeClipboardContentFromMonitorProcessing` and `CanIncludeInClipboardHistory = 0`), `WinKeyboard: Keyboard` (`SendInput` Ctrl down, V down, V up, Ctrl up; `is_down` via `GetAsyncKeyState`), `CpalRecorder: Recorder` (default input device, converts via `to_whisper_input`, maps no-device and mid-stream errors to `AudioError`), `pub fn play_cue(cue: Cue)` (`MessageBeep` or short `PlaySound`, no asset files).

- [ ] **Step 1:** `cargo check --no-default-features --target x86_64-pc-windows-gnu` must pass.
- [ ] **Step 2:** on Windows, write a throwaway `examples/smoke_output.rs` that records 3 s, then pastes "hello" into Notepad; run it by hand and confirm. Delete the example afterward (not committed).
- [ ] **Step 3: Commit** `feat: add windows clipboard, keyboard, recorder`.

### Task 11: Tray, startup, single instance, main, release build

**Files:** Create `src/platform/{tray,autostart,instance}.rs`, `src/main.rs`, `README.md`

**Interfaces:**
- Produces: tray menu with status icon (four `TrayState` icons), "Set hotkey", model size (base/small), "Start with Windows" (HKCU `Run` key via `autostart::set(enabled: bool) -> Result<(), PlatformError>`), "Open config", "Quit". `instance::acquire() -> Result<InstanceGuard, InstanceError>` using a named mutex; a second launch exits with a notification. `main` loads config from `%APPDATA%\wdys\config.toml`, runs `ensure_model` (consent via a Windows message box), installs the hook, starts the ticker, runs `run_worker` on a worker thread, and pumps the tray/message loop on the main thread. Model idle-unload is checked on every `Tick`.

- [ ] **Step 1:** `cargo check --no-default-features --target x86_64-pc-windows-gnu` passes; on Windows `cargo build --release` succeeds.
- [ ] **Step 2: Manual verification on Windows** (write this checklist into `README.md` and tick it off with the user): first run asks before downloading, declining leaves a clear tray notice; hold Right Ctrl, speak, release and text appears in Notepad and in a browser text box; "um" and "uh" are gone; a prior clipboard text is restored; double-press hands-free then a single press stops it; a tap does nothing; "Set hotkey" to F9 works and persists after restart; a second launch is refused; idle memory after 10 minutes is under 50 MB (exclude the model) and Task Manager CPU is about 0%; unplugging the microphone mid-recording shows a notice; Quit exits cleanly.
- [ ] **Step 3: Commit** `feat: add tray app, autostart, single instance` and `docs: add readme and manual test checklist`.
