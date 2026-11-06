# wdys: design notes

> This is the design document written before the first implementation, kept for background. Where it differs
> from the code or the README (for example the default hotkey, now F9, and the hotkey being blocked), the code
> and README are current. It was written in a collaboration between the author and an AI assistant, which is why
> some decisions are attributed to "the user" and "Claude".

## Intent and why

The user dislikes typing into apps, especially AI chat tools, and wants to
speak instead. They need this constantly, all day. The tool is a background
Windows utility: press a key, speak, and clean text is pasted into whatever
app currently has focus.

## Goals

- Runs in the background, managed entirely from the system tray.
- Very lightweight: small single executable, minimal idle memory and CPU, no
  heavyweight runtime (no Python, no Electron).
- Fully local: audio and text never leave the machine. No network use for
  transcription.
- Hold-to-talk: hold the hotkey to record, release to transcribe and paste.
- Hands-free mode: double-press the hotkey to start continuous recording.
- Filler words with no meaning ("um", "uh", "ah", and similar) are removed
  from the pasted text.
- Hotkey is user-defined.
- English only.

## Non-goals

- Other languages.
- GPU acceleration (possible later; not in v1).
- Rewriting, summarizing, or grammar-correcting speech with a language model.
- Cloud transcription.
- Voice commands ("new line", "delete that"), punctuation commands.
- Cross-platform support. Windows only.

## Anti-goals (failure even if it technically works)

- Noticeable idle resource use, or slow startup of each dictation.
- Any audio or text leaving the machine.
- Clobbering the user's clipboard permanently after a paste.
- The hotkey leaking into the app being dictated into (stray characters or shortcuts).
- Recording that gets stuck on with no way to tell.

## Decisions made by the user

- Local-only transcription.
- Hold the key to record; double-press for hands-free.
- The hotkey is user-defined (set from the tray).
- The hotkey IS blocked: while wdys runs, the hotkey is swallowed and never reaches any app
  (changed from an earlier "don't block it" decision, at the user's request). Consequence: that
  key stops working normally everywhere, so pick one you never use.
- English only.
- The builder (Claude) chooses language and toolchain, within the
  lightweight and local constraints.

## Decisions made by Claude (the user has not specifically approved these)

- **Language:** Rust. Single small `.exe`, no runtime, strong typing.
- **Transcription:** whisper.cpp via the `whisper-rs` bindings, using an
  English-only Whisper model. Default: "base.en" (~150 MB). Model size is a
  setting (e.g. "small.en" for more accuracy).
- **CPU only** in v1.
- **Paste mechanism:** put text on the clipboard, simulate Ctrl+V, then
  restore the previous clipboard contents.
- **Filler removal:** rule-based text post-processing in code (a fixed list
  of filler words and stuttered repeats). No second AI model. Known limit:
  rules do not fix false starts or rambling sentences.
- **Model lifecycle:** load the model on first use; unload after an idle
  timeout to keep memory low.
- **Feedback:** tray icon changes state (idle / recording / hands-free /
  transcribing), plus a short sound on start and stop.
- **Hands-free end:** a further press of the hotkey ends it. A configurable
  maximum recording length also ends it, so it can never run indefinitely.
- **Startup:** tray toggle "Start with Windows", off by default.
- **Hotkey setup:** tray item "Set hotkey" — click, press the desired key, it
  is saved to a config file and applied immediately. Because the key is
  blocked system-wide, pick one you never use (an F-key, Scroll Lock);
  a typing key or a modifier like Ctrl would break normal use. Default: F9.
- **Config:** a small config file (hotkey, model size, max recording length,
  idle unload timeout, sound on/off, start-with-Windows).
- **Dependencies (new; to be mentioned at implementation time):** `whisper-rs`,
  `cpal` (microphone capture), `tray-icon` (system tray), `windows` crate
  (global key hook, keystroke simulation, clipboard), `thiserror` (custom
  error types).

## Model acquisition (decided by the user)

The model file is downloaded on first run, after asking the user's
permission. That download is the only network use. A manual-placement
fallback (user puts the model file in the expected folder) is also
supported. Audio and text never leave the machine. The download should be
checksum-verified.

## Further calls by Claude (after builder review; not yet approved by the user)

- **Hotkey types:** keyboard keys only in v1 (no mouse buttons). Key
  auto-repeat is collapsed to one key-down and swallowed with the hotkey.
  Injected (software-generated) key events, such as our own Ctrl+V, are never
  blocked or acted on.
  Paste is always sent after the hotkey has been released, and the paste
  step waits for the hotkey to be up.
- **Tap vs double-press:** recording starts immediately on key-down (lowest
  latency). A hold shorter than a minimum (default ~0.3 s) is discarded as a
  tap. Two taps within the double-press window promote to hands-free.
- **Output text:** Whisper's native capitalization and punctuation, no
  trailing space or newline. Hands-free transcribes once at the end (no
  incremental pasting, no chunking in v1).
- **Latency/memory:** model stays loaded for a default idle timeout of
  10 minutes, then unloads. Target hardware: an ordinary modern laptop CPU.
- **Paste failure never loses text (added at the user's request):** if the paste cannot
  be completed (hotkey still held, input blocked), the dictated text is left on the clipboard
  as a normal entry and a toast says so. The latest transcript is also kept in memory only
  (never written to disk) and the tray offers "Copy last transcript". Only the last one is kept.
- **Clipboard:** v1 saves and restores text content only; non-text clipboard
  contents (images, files) are not preserved and this is a known limitation.
  The dictated text is marked to be excluded from Windows clipboard history.
  Paste into elevated (administrator) windows is not supported.
- **Audio device:** default input device only. Missing or disconnected
  device surfaces a tray notification and ends the recording.
- **Stuck-recording safeguard:** the maximum recording length applies to both
  hold-to-talk and hands-free (default 120 s), which is what ends a recording
  whose key-up was lost. (OS key-state polling is not used: a swallowed key never
  registers with Windows, so `GetAsyncKeyState` would always report it up.)
- **Build:** release builds are made on Windows. Logic sits behind traits so
  it tests on Linux/WSL. Only one instance may run at a time.

## Constraints

- Windows target. Development happens in Linux/WSL, so logic must be
  testable without Windows APIs or a microphone; hotkey, microphone, and
  paste behavior need verification on real Windows.
- Engineering conventions from the user's global rules: test-driven
  development; integration tests preferred for business logic; functional
  style over classes where practical; full type annotations, no `any`
  equivalents; custom error types rather than string errors; conventional
  commit messages.
- Hold-to-talk and double-press require key-up/key-down detection, which
  means a global low-level keyboard hook rather than a registered hotkey.

## Behavior summary

| Action | Result |
|---|---|
| Press and hold hotkey | Start recording; tray icon shows recording |
| Release hotkey | Stop, transcribe, remove fillers, paste into focused app |
| Double-press hotkey | Enter hands-free recording |
| Press hotkey during hands-free | Stop, transcribe, clean, paste |
| Max length reached in hands-free | Stop, transcribe, clean, paste |
| Tray: Set hotkey | Capture next key as the new hotkey |
| Tray: Start with Windows | Toggle autostart |
| Tray: Quit | Exit, releasing the hook and model |

## Left to the builder

- Exact double-press timing window (should be configurable with a sensible
  default).
- Exact filler word list and stutter handling rules.
- Audio resampling details (Whisper expects 16 kHz mono).
- Project layout, module boundaries, and test strategy.
- How to distinguish a short hold-to-talk tap from the first half of a
  double-press.
- Behavior when the microphone is missing or the model file is absent
  (should surface a clear tray notification, via custom error types).
- How the model file is obtained (bundled, or downloaded on first run with
  the user's consent).
