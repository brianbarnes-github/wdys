# Manual verification checklist (Windows)

The logic is covered by automated tests (`cargo test --no-default-features`), but the Windows layer (keyboard
hook, clipboard, microphone, tray, toasts, overlay) can only be verified on a real machine. Run through this
list after building a release or changing that code.

- [ ] First run asks before downloading the model; declining shows a notice in the tray tooltip and does not crash.
- [ ] Hold F9, speak, release: text appears in Notepad and in a browser text box, and no F9 keypress reaches the app (e.g. F9 does nothing in a browser).
- [ ] "um" and "uh" are gone from the pasted text.
- [ ] Text that was on the clipboard before dictating is back on the clipboard afterwards.
- [ ] Double-press starts hands-free: a small pill with a pulsing red dot and the mic icon appears at the top centre of the screen (and the tray dot turns orange); one more press hides it, stops, and pastes.
- [ ] A quick tap does nothing.
- [ ] Tray > "Copy last transcript" puts the last dictation on the clipboard (Ctrl+V pastes it) and shows a toast; before any dictation it says there is nothing to copy.
- [ ] Hold the hotkey past the end of a recording (or paste into a window that blocks input): a toast says the text is on the clipboard, and Ctrl+V pastes it.
- [ ] Tray > "Set hotkey...": a "Select your key" toast appears; press a key and a "Hotkey set" toast names it. The new key works, and still does after restarting wdys.
- [ ] A second launch is refused with a message.
- [ ] Unplugging the microphone mid-recording shows a notice in the tray tooltip and wdys recovers.
- [ ] Idle for 10+ minutes: Task Manager shows ~0% CPU and memory drops (model unloaded).
- [ ] Tray > "Start with Windows" survives a sign-out and sign-in.
- [ ] Tray > Quit exits cleanly (no leftover process).
