#!/usr/bin/env bash
# Cross-builds wdys.exe for Windows from Ubuntu/WSL and optionally copies it to a folder.
#   usage: scripts/build-windows-from-wsl.sh [/mnt/c/Tools/wdys]
# Needs: rustup target x86_64-pc-windows-gnu, mingw-w64 (with the -posix compilers), cmake, libclang.
# Works around Ubuntu 24.04's mingw-w64 v11: it lacks THREAD_POWER_THROTTLING_STATE (needed by
# whisper.cpp) and names the CMake static libs `ggml.a` instead of `libggml.a`.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=target/x86_64-pc-windows-gnu/release
WORK="$PWD/target/winbuild"
mkdir -p "$WORK/staticlibs"

cat > "$WORK/mingw_shim.h" <<'HDR'
#include <windows.h>
#ifndef THREAD_POWER_THROTTLING_CURRENT_VERSION
#define THREAD_POWER_THROTTLING_CURRENT_VERSION 1
#define THREAD_POWER_THROTTLING_EXECUTION_SPEED 0x1
typedef struct _THREAD_POWER_THROTTLING_STATE { ULONG Version; ULONG ControlMask; ULONG StateMask; } THREAD_POWER_THROTTLING_STATE;
#endif
HDR
# Link libstdc++ statically so the exe needs no extra DLLs.
ln -sf /usr/lib/gcc/x86_64-w64-mingw32/*-posix/libstdc++.a "$WORK/staticlibs/libstdc++.a"

export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-ar-posix
export CFLAGS_x86_64_pc_windows_gnu="-include $WORK/mingw_shim.h"
export CXXFLAGS_x86_64_pc_windows_gnu="-include $WORK/mingw_shim.h"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix
export RUSTFLAGS="-C link-arg=-static -C link-arg=-L$WORK/staticlibs"

build() { cargo build --release --target x86_64-pc-windows-gnu; }
link_names() {
  for dir in target/x86_64-pc-windows-gnu/release/build/whisper-rs-sys-*/out/build/ggml/src; do
    for n in ggml ggml-base ggml-cpu; do [ -e "$dir/$n.a" ] && ln -sf "$n.a" "$dir/lib$n.a"; done
  done
}
# First pass builds whisper.cpp (then fails to link by name); fix names; second pass finishes.
build >/dev/null 2>&1 || true
link_names
build

echo "built: $OUT/wdys.exe"
if [ "${1:-}" ]; then
  mkdir -p "$1" && cp "$OUT/wdys.exe" "$1/wdys.exe" && echo "copied to $1"
fi
