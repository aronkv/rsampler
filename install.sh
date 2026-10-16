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

# Adds rsampler to the desktop's app launcher, with its icon. Optional: a failure here only
# skips the launcher entry.
desktop_entry() {
    [ "$(uname -s)" = Linux ] || return 0
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    raw="https://raw.githubusercontent.com/$repo/main/assets"
    dir=$(mktemp -d)
    if curl -fsSL "$raw/rsampler.desktop" -o "$dir/rsampler.desktop" \
        && curl -fsSL "$raw/rsampler.svg" -o "$dir/rsampler.svg" \
        && curl -fsSL "$raw/rsampler.png" -o "$dir/rsampler.png"; then
        mkdir -p "$data/applications" "$data/icons/hicolor/scalable/apps" "$data/icons/hicolor/256x256/apps"
        # Launchers often run without the shell's PATH, so point at the binary directly.
        sed "s|^Exec=.*|Exec=$1|" "$dir/rsampler.desktop" > "$data/applications/rsampler.desktop"
        install -m 644 "$dir/rsampler.svg" "$data/icons/hicolor/scalable/apps/rsampler.svg"
        install -m 644 "$dir/rsampler.png" "$data/icons/hicolor/256x256/apps/rsampler.png"
        echo "Added rsampler to the app launcher"
    else
        echo "Could not download the launcher entry; skipped it" >&2
    fi
    rm -rf "$dir"
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
    desktop_entry "${CARGO_HOME:-$HOME/.cargo}/bin/rsampler"
    path_hint "${CARGO_HOME:-$HOME/.cargo}/bin"
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
bin_dir=$(cd "$bin_dir" && pwd)
install -m 755 "$tmp/rsampler" "$bin_dir/rsampler"
echo "Installed to $bin_dir/rsampler"
desktop_entry "$bin_dir/rsampler"
path_hint "$bin_dir"
