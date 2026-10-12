#!/usr/bin/env bash
# Downloads the Zig compiler into tools/zig/, which is only used as the linker for building the
# Windows version from Linux (see build.sh). Nothing is installed system-wide.
set -euo pipefail
cd "$(dirname "$0")"

VERSION=0.14.1
URL="https://ziglang.org/download/$VERSION/zig-x86_64-linux-$VERSION.tar.xz"
SHA256=24aeeec8af16c381934a6cd7d95c807a8cb2cf7df9fa40d359aa884195c4716c

if [ -x "zig/zig-x86_64-linux-$VERSION/zig" ]; then
    echo "zig $VERSION is already in tools/zig"
    exit 0
fi
rm -rf zig
mkdir zig
curl -fL --progress-bar -o zig/zig.tar.xz "$URL"
echo "$SHA256  zig/zig.tar.xz" | sha256sum -c -
tar -xJf zig/zig.tar.xz -C zig
rm zig/zig.tar.xz
echo "zig $VERSION is in tools/zig"
