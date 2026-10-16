#!/bin/sh
# Installs rsampler. On Linux x86_64 it downloads the latest prebuilt release; elsewhere it builds
# from source with cargo.
# Usage: curl -fsSL https://raw.githubusercontent.com/aronkv/rsampler/main/install.sh | sh
set -eu

repo=aronkv/rsampler
bin_dir="${RSAMPLER_INSTALL_DIR:-$HOME/.local/bin}"

path_hint() {
    case ":$PATH:" in
        *":$1:"*) echo "Done. Start it with: rsampler" ;;
        *) echo "Done. Add $1 to your PATH, then start it with: rsampler" ;;
    esac
}

from_source() {
    if ! command -v cargo >/dev/null 2>&1; then
        echo "No prebuilt rsampler for this system; building it needs the Rust toolchain." >&2
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
    cargo install --locked --git "https://github.com/$repo" rsampler
    path_hint "$HOME/.cargo/bin"
}

if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
    from_source
    exit
fi

asset=rsampler-x86_64-unknown-linux-gnu.tar.gz
url="https://github.com/$repo/releases/latest/download/$asset"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $asset"
curl -fsSL "$url" -o "$tmp/$asset"
curl -fsSL "$url.sha256" -o "$tmp/$asset.sha256"
(cd "$tmp" && sha256sum -c --quiet "$asset.sha256")
tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$bin_dir"
install -m 755 "$tmp/rsampler" "$bin_dir/rsampler"
echo "Installed to $bin_dir/rsampler"
path_hint "$bin_dir"
