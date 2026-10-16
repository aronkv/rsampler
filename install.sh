#!/bin/sh
# Builds and installs rsampler from source into ~/.cargo/bin.
# Usage: curl -fsSL https://raw.githubusercontent.com/aronkv/rsampler/main/install.sh | sh
set -eu

if ! command -v cargo >/dev/null 2>&1; then
    echo "rsampler is built from source and needs the Rust toolchain." >&2
    echo "Install it from https://rustup.rs, then run this script again." >&2
    exit 1
fi

# The audio backend links against ALSA on Linux.
if [ "$(uname -s)" = Linux ] && ! pkg-config --exists alsa 2>/dev/null; then
    echo "Missing ALSA development files. Install them first:" >&2
    echo "  Arch:          sudo pacman -S alsa-lib pkgconf" >&2
    echo "  Debian/Ubuntu: sudo apt install libasound2-dev pkg-config" >&2
    echo "  Fedora:        sudo dnf install alsa-lib-devel pkgconf-pkg-config" >&2
    exit 1
fi

cargo install --locked --git https://github.com/aronkv/rsampler rsampler

case ":$PATH:" in
    *":$HOME/.cargo/bin:"*) echo "Done. Start it with: rsampler" ;;
    *) echo "Done. Add ~/.cargo/bin to your PATH, then start it with: rsampler" ;;
esac
