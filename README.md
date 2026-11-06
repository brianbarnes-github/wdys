# wdys

A tiny background voice-dictation tool for Windows. Hold a key, speak, release: your words are
transcribed locally (Whisper, no cloud), filler words ("um", "uh", "ah") are removed, and the
text is pasted into whatever app has focus.

- **Hold the hotkey** to record, **release** to transcribe and paste.
- **Double-press** the hotkey for hands-free recording; press once more to stop.
- Lives in the system tray; the hotkey, model size and "Start with Windows" are set from its menu.
- Default hotkey: Right Ctrl. English only. CPU only. Audio and text never leave your machine.
- The only network use is a one-time model download (asks first, checksum-verified).

Settings live in `%APPDATA%\wdys\config.toml`; models in `%APPDATA%\wdys\models`.
To skip the download, place `ggml-base.en.bin` (or `ggml-small.en.bin`) from
<https://huggingface.co/ggerganov/whisper.cpp> in that folder.

## Building (on Windows)

Prerequisites: Rust (MSVC toolchain), Visual Studio Build Tools (C++), CMake, and LLVM (for `libclang`).

```
cargo build --release
```

The executable is `target\release\wdys.exe`.

Logic tests run anywhere (including Linux/WSL): `cargo test --no-default-features`.

## Manual verification checklist (Windows)

- [ ] First run asks before downloading the model; declining shows a notice in the tray tooltip and does not crash.
- [ ] Hold Right Ctrl, speak, release: text appears in Notepad and in a browser text box.
- [ ] "um" and "uh" are gone from the pasted text.
- [ ] Text that was on the clipboard before dictating is back on the clipboard afterwards.
- [ ] Double-press starts hands-free (tray icon turns orange); one more press stops and pastes.
- [ ] A quick tap does nothing.
- [ ] Tray > "Set hotkey...", press F9: F9 now works, and it still works after restarting wdys.
- [ ] A second launch is refused with a message.
- [ ] Unplugging the microphone mid-recording shows a notice in the tray tooltip and wdys recovers.
- [ ] Idle for 10+ minutes: Task Manager shows ~0% CPU and memory drops (model unloaded).
- [ ] Tray > "Start with Windows" survives a sign-out and sign-in.
- [ ] Tray > Quit exits cleanly (no leftover process).
