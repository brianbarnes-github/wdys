# Contributing to wdys

Thanks for helping! Bug reports, ideas and pull requests are all welcome.

## Reporting a bug

Open an issue and include your Windows version, what you did, what you expected, and what happened. If the tray
icon showed a yellow dot or a toast appeared, include its text. Don't attach recordings of private speech.

## Making a change

The project is Rust. The logic is plain functions and small traits so it can be tested anywhere; the Windows-only
code lives in `src/platform/`.

```bash
cargo fmt
cargo clippy --no-default-features --all-targets -- -D warnings
cargo test --no-default-features
cargo clippy --no-default-features --target x86_64-pc-windows-gnu --all-targets -- -D warnings   # Windows layer
```

Guidelines:

- **Write the test first** for new behaviour, and prefer integration-style tests (see `tests/app.rs`) over tests of
  internals. Put decisions in pure functions so they can be tested without Windows or a microphone.
- Prefer small functions and plain data over classes; keep types explicit and use the custom error types in
  `src/error.rs` rather than strings.
- Mention any new dependency in your pull request, and keep the app small and local-only: nothing should ever send
  audio or text off the machine.
- Changes to `src/platform/` can't be unit-tested; describe how you checked them on Windows and tick the relevant
  items in [docs/MANUAL_TESTING.md](docs/MANUAL_TESTING.md).
- Use [conventional commit](https://www.conventionalcommits.org/) messages (`feat:`, `fix:`, `docs:` ...).

## Building the Windows exe

See "Building from source" in the [README](README.md). Releases are built by the GitHub Actions workflow in
`.github/workflows/release.yml` when a `v*` tag is pushed.

## License

By contributing you agree that your contribution is licensed under the [MIT license](LICENSE).
