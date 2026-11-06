#!/usr/bin/env python3
"""Regenerates THIRD_PARTY_NOTICES.md from the Cargo dependency graph.

usage: python3 scripts/gen-third-party.py   (run from the repo root; needs cargo and network-free metadata)
"""
import glob
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-pc-windows-gnu"


def metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET, "--features", "whisper"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    )
    return json.loads(out.stdout)


def runtime_closure(meta: dict) -> list[dict]:
    """Packages that end up in (or are built for) the release binary; dev-dependencies excluded."""
    by_id = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = meta["resolve"]["root"]
    seen, stack = set(), [root]
    while stack:
        pkg = stack.pop()
        if pkg in seen:
            continue
        seen.add(pkg)
        stack.extend(d["pkg"] for d in nodes[pkg]["deps"] if any(k["kind"] in (None, "build") for k in d["dep_kinds"]))
    return sorted((by_id[i] for i in seen if i != root), key=lambda p: (p["name"], p["version"]))


def whisper_cpp_license() -> str:
    paths = glob.glob(str(Path.home() / ".cargo/registry/src/*/whisper-rs-sys-*/whisper.cpp/LICENSE"))
    if not paths:
        raise SystemExit("whisper.cpp LICENSE not found; run `cargo fetch` first")
    return Path(sorted(paths)[-1]).read_text().strip()


def main() -> None:
    crates = runtime_closure(metadata())
    rows = "\n".join(
        f"| {p['name']} | {p['version']} | {p.get('license') or 'see repository'} | {p.get('repository') or ''} |"
        for p in crates
    )
    text = f"""# Third-party notices

wdys itself is MIT licensed (see `LICENSE`). It includes or downloads the following third-party work.

## Speech recognition

- **Whisper** (OpenAI): the speech recognition model wdys downloads on first run
  (`ggml-base.en.bin` / `ggml-small.en.bin`, converted for whisper.cpp by its maintainers). MIT License,
  Copyright (c) 2022 OpenAI. The model is **not** bundled with wdys; it is downloaded from
  <https://huggingface.co/ggerganov/whisper.cpp> after you agree.
- **whisper.cpp / ggml**: the C/C++ inference engine compiled into `wdys.exe` (via the `whisper-rs` bindings).
  MIT License:

```
{whisper_cpp_license()}
```

## Artwork

The application icon (`resources/wdys-icon.png` and files generated from it) is covered by the project's MIT license.

## Rust crates

These crates are compiled into `wdys.exe` or used to build it. All are under permissive licenses; each crate's
license text is available in its repository and in its package on <https://crates.io>.

| Crate | Version | License | Repository |
| --- | --- | --- | --- |
{rows}
"""
    (ROOT / "THIRD_PARTY_NOTICES.md").write_text(text)
    print(f"wrote THIRD_PARTY_NOTICES.md ({len(crates)} crates)")


if __name__ == "__main__":
    main()
