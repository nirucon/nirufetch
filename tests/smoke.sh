#!/bin/sh
set -eu
BIN=${1:?usage: smoke.sh BIN}
[ "$("$BIN" --version)" = "NIRUFETCH 0.1.0" ]
out=$("$BIN" --plain)
printf '%s\n' "$out" | grep -q '^OS'
printf '%s\n' "$out" | grep -q 'Kernel'
printf '%s\n' "$out" | grep -q 'Memory'
if printf '%s\n' "$out" | grep -q "$(printf '\033')"; then
  echo 'FAIL: plain output contains ANSI escapes' >&2
  exit 1
fi
echo 'PASS: version, plain output and no ANSI escapes'
