#!/usr/bin/env bash
# Builds release versions of the game and the host for Linux and for Windows (64 bit), and
# packs each into a zip in dist/.
#
#   ./build.sh            both
#   ./build.sh linux      just Linux
#   ./build.sh windows    just Windows (cross compiled with zig, see tools/get-zig.sh)
set -euo pipefail
cd "$(dirname "$0")"

WHAT="${1:-all}"
VERSION="$(git describe --tags --always --dirty 2>/dev/null || echo dev)"
BINS=(gamedemo gamedemo-host)
mkdir -p dist

pack() { # pack <target dir> <exe suffix> <zip name> [extra files...]
    local dir="$1" suffix="$2" name="$3"
    shift 3
    local stage="dist/stage-$name"
    rm -rf "$stage" "dist/$name.zip"
    mkdir -p "$stage/$name"
    for bin in "${BINS[@]}"; do
        cp "$dir/$bin$suffix" "$stage/$name/"
    done
    cp README.md "$stage"/"$name"/
    for extra in "$@"; do
        cp "$extra" "$stage/$name/"
    done
    (cd "$stage" && zip -qr "../$name.zip" "$name")
    rm -rf "$stage"
    echo "  dist/$name.zip ($(du -h "dist/$name.zip" | cut -f1))"
}

if [ "$WHAT" = all ] || [ "$WHAT" = linux ]; then
    echo "== Linux"
    cargo build --release -p gamedemo -p gamedemo-host
    pack target/release "" "gamedemo-$VERSION-linux-x86_64"
fi

if [ "$WHAT" = all ] || [ "$WHAT" = windows ]; then
    echo "== Windows"
    tools/get-zig.sh
    export PATH="$PWD/tools/zig/zig-x86_64-linux-0.14.1:$PATH"
    command -v cargo-zigbuild >/dev/null || cargo install cargo-zigbuild --locked
    rustup target add x86_64-pc-windows-gnu
    cargo zigbuild --release --target x86_64-pc-windows-gnu -p gamedemo -p gamedemo-host
    pack target/x86_64-pc-windows-gnu/release .exe "gamedemo-$VERSION-windows-x86_64" \
        tools/windows/join-game.bat tools/windows/host-game.bat
fi
echo "done"
