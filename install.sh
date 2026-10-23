#!/bin/sh
set -eu
VERSION="0.1.2"
REPO="${NIRUFETCH_REPO:-nirucon/nirufetch}"
PREFIX="${NIRUFETCH_PREFIX:-${NIRU_PREFIX:-$HOME/.local}}"
BIN="$PREFIX/bin"
need(){ command -v "$1" >/dev/null 2>&1; }
say(){ printf '%s\n' "$*"; }
die(){ say "nirufetch install: $*" >&2; exit 1; }
fetch(){ need curl && curl -fsSL "$1" -o "$2" || wget -qO "$2" "$1" || die "download failed"; }
managed_guard() {
  [ -L "$BIN/nirufetch" ] || return 0
  case "$(readlink "$BIN/nirufetch")" in
    */share/nirux/current/bin/nirufetch) die 'NIRUFETCH is managed by NIRUX; use nirux update.' ;;
  esac
}
managed_guard
case "${1:-}" in
  --build) need cargo || die 'cargo required'; (cd "$(dirname "$0")" && cargo build --release --locked); install -d "$BIN"; install -m755 "$(dirname "$0")/target/release/nirufetch" "$BIN/nirufetch" ;;
  --uninstall) rm -f "$BIN/nirufetch" ;;
  "") d=$(mktemp -d); trap 'rm -rf "$d"' EXIT; fetch "https://github.com/$REPO/releases/download/v$VERSION/nirufetch" "$d/nirufetch"; fetch "https://github.com/$REPO/releases/download/v$VERSION/SHA256SUMS" "$d/SHA256SUMS"; sum=$(sha256sum "$d/nirufetch" | cut -d' ' -f1); grep -q "^$sum  nirufetch$" "$d/SHA256SUMS" || die 'checksum mismatch'; install -d "$BIN"; install -m755 "$d/nirufetch" "$BIN/nirufetch"; say "Installed NIRUFETCH $VERSION" ;;
  *) die "unknown option $1" ;;
esac
