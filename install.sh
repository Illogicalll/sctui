#!/bin/sh
# sctui installer for macOS and Linux — https://github.com/Illogicalll/sctui
#
#   curl -fsSL https://raw.githubusercontent.com/Illogicalll/sctui/main/install.sh | sh
#
# Environment:
#   SCTUI_VERSION=v0.1.0      install a specific release (default: latest)
#   SCTUI_INSTALL_DIR=<dir>   where to put the binary (default: ~/.local/bin)
set -eu

REPO="Illogicalll/sctui"
INSTALL_DIR="${SCTUI_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*" >&2; }
die() { say "error: $*"; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed"; }

need curl
need tar

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Darwin) os_target=apple-darwin ;;
  Linux) os_target=unknown-linux-gnu ;;
  MINGW*|MSYS*|CYGWIN*) die "on Windows run:  irm https://raw.githubusercontent.com/$REPO/main/install.ps1 | iex" ;;
  *) die "unsupported operating system: $os" ;;
esac
case "$arch" in
  x86_64|amd64) arch_target=x86_64 ;;
  arm64|aarch64) arch_target=aarch64 ;;
  *) die "unsupported architecture: $arch" ;;
esac
target="$arch_target-$os_target"

version="${SCTUI_VERSION:-}"
if [ -z "$version" ]; then
  version=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
  [ -n "$version" ] || die "could not find the latest release (none published yet, or GitHub API unreachable)"
fi

name="sctui-$target"
base="https://github.com/$REPO/releases/download/$version"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

say "Downloading sctui $version for $target..."
curl -fsSL -o "$tmp/$name.tar.gz" "$base/$name.tar.gz" \
  || die "no build for $target in release $version (see https://github.com/$REPO/releases)"
curl -fsSL -o "$tmp/$name.tar.gz.sha256" "$base/$name.tar.gz.sha256" \
  || die "checksum file missing for $version"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$tmp" && sha256sum -c --quiet "$name.tar.gz.sha256") || die "checksum mismatch"
elif command -v shasum >/dev/null 2>&1; then
  (cd "$tmp" && shasum -a 256 -c --quiet "$name.tar.gz.sha256") || die "checksum mismatch"
else
  say "warning: neither sha256sum nor shasum found, skipping checksum verification"
fi

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$INSTALL_DIR"
install -m 755 "$tmp/$name/sctui" "$INSTALL_DIR/sctui"
say "Installed sctui $version to $INSTALL_DIR/sctui"

if [ "$os" = Linux ]; then
  ldconfig_bin=$(command -v ldconfig 2>/dev/null || echo /sbin/ldconfig)
  if [ -x "$ldconfig_bin" ] && ! "$ldconfig_bin" -p 2>/dev/null | grep -q 'libasound\.so\.2'; then
    say "note: ALSA runtime (libasound2) not found. Install it with your package manager,"
    say "      e.g. 'sudo apt install libasound2' or 'sudo dnf install alsa-lib'."
  fi
fi

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    say "note: $INSTALL_DIR is not on your PATH. Add this line to your shell profile:"
    say "      export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

say "Run 'sctui' to start. The first launch opens your browser to sign in to SoundCloud."
