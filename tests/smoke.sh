#!/bin/sh
set -eu
BIN=${1:?usage: smoke.sh BIN}
[ "$("$BIN" --version)" = "NIRUFETCH 0.2.0" ]
out=$("$BIN" --plain)
printf '%s\n' "$out" | grep -q '^OS'
printf '%s\n' "$out" | grep -q 'Kernel'
printf '%s\n' "$out" | grep -q 'Memory'
if printf '%s\n' "$out" | grep -q "$(printf '\033')"; then
  echo 'FAIL: plain output contains ANSI escapes' >&2
  exit 1
fi
for flags in '' --classic --compact --full '--classic --full' --no-color --no-logo --plain; do
  # shellcheck disable=SC2086
  "$BIN" $flags >/dev/null || { echo "FAIL: nirufetch $flags" >&2; exit 1; }
done
if NO_COLOR=1 "$BIN" --full | grep -q "$(printf '\033')"; then
  echo 'FAIL: NO_COLOR output contains ANSI escapes' >&2
  exit 1
fi
json=$("$BIN" --json)
printf '%s\n' "$json" | grep -q '"nirufetch": "0.2.0"'
if command -v python3 >/dev/null 2>&1; then
  printf '%s\n' "$json" | python3 -c 'import json, sys; json.load(sys.stdin)'
fi
# Runtime budget: a run starts no child process (execve only for itself).
if command -v strace >/dev/null 2>&1; then
  trace=$(mktemp)
  trap 'rm -f "$trace"' EXIT
  for flags in '' --full --json; do
    # shellcheck disable=SC2086
    if strace -f -qq -e trace=execve,fork,vfork,clone,clone3 -o "$trace" "$BIN" $flags >/dev/null 2>&1; then
      calls=$(grep -cE 'execve\(|fork\(|clone3?\(' "$trace" || true)
      if [ "$calls" != 1 ]; then
        echo "FAIL: nirufetch $flags made $calls execve/fork/clone calls (expected 1)" >&2
        cat "$trace" >&2
        exit 1
      fi
    else
      echo 'SKIP: strace is not permitted here' >&2
      break
    fi
  done
  echo 'PASS: no child processes (strace)'
fi
echo 'PASS: version, flags, plain output, JSON and no ANSI escapes'
