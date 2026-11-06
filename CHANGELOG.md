# Changelog

## 0.1.0

First public release.

- Hold-to-talk dictation with a user-defined hotkey (default F9); the hotkey is blocked from reaching other apps.
- Hands-free mode (double-press) with an on-screen pulsing indicator.
- Local transcription with Whisper (`base.en` by default, `small.en` selectable), one-time model download with
  consent and checksum verification.
- Filler-word, stutter and Whisper-marker removal.
- Paste via the clipboard with restore; a failed paste leaves the text on the clipboard, and "Copy last
  transcript" is available from the tray.
- System tray app with status icon, toasts, start-with-Windows, single-instance guard, idle model unload.
