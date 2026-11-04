# Agent assignment: NIRUFETCH 0.2.0, a one-shot accuracy and output update

## 0. Quick start

1. Read this file, `README.md`, `src/main.rs` and `tests/smoke.sh`. The audit baseline is
   `1b4d3d6` (0.1.4, pinned by NIRUX 0.12.199). Revalidate each item against current HEAD
   and skip items that are already fixed, noting that you did.
2. Implement **F1–F6, then U1–U3 in order**. Commit per item. Keep fmt, clippy
   `-D warnings`, `cargo test --locked` and `tests/smoke.sh` green.
3. Pass the gates in §4, release in §5 and report in §6.

This is a small app with a good base: a run takes about 2 ms and it starts only one
process. Keep it that way. This is an **execution** assignment: do not stop after a plan,
and continue without routine questions.

## 1. Invariants

- No network, no daemon, and no shell pipelines. It reads local files, with at most a few
  bounded helper calls only where no file source exists.
- **A run stays at ≤ 10 ms and ≤ 1 child process on a normal system.** Every new field
  must fit inside that budget.
- Only `libc` as a dependency. Keep `rust-version = 1.85`.
- Existing flags keep working: `--classic --compact --full --no-color --no-logo --plain`.
- Color is off when stdout is not a TTY or `NO_COLOR` is set. That already works today;
  keep it.

## 2. Findings (verified 2026-11-04 on niruxwork)

### F1: `--classic` labels are not aligned
`--classic` prints `OS │ …`, `Kernel │ …`, `Terminal │ …` with the separator directly
after each label, so the separators zigzag. **Fix:** pad the labels to the widest label,
like the default layout does. Add a test that checks for a straight separator column.

### F2: The terminal is guessed from `$TERM`
The default output shows `kitty`, `--full` shows `xterm-kitty`, and `TERM=dumb` shows
`dumb`. **Fix:** find the terminal by walking parent processes (`/proc/<pid>/stat` +
`comm`/`exe`) up to the first known terminal emulator (kitty, alacritty, foot, wezterm,
xterm, st, urxvt, gnome-terminal, konsole, the NIRUX terminal…). Fall back to
`$TERM_PROGRAM`, then `$TERM`. Show the same name in every mode.

### F3: An unnecessary process for the kernel
It runs `uname -r` as a child process. **Fix:** use `libc::uname`. Then a normal run
starts no processes at all.

### F4: The NIRUX fields are vague
`WM  NIRU` and `Theme  niru-forest-vigil`. **Fix:**
- WM shows `NIRUWM 0.16.167`, read from the installed generation's `manifest`
  (`~/.local/share/nirux/current/manifest`) without running anything.
- Add a `NIRUX 0.12.199` row.
- Theme shows the theme's display name ("Forest Vigil") from the theme catalog files, with
  the id only in `--full`.

### F5: Truncation without `--full`
The CPU model is cut to `AMD Ryzen AI 9 HX PRO 375 w/ Rade…`. **Fix:** strip vendor
noise first (`w/ Radeon 890M`, `(R)`, `(TM)`, "CPU @ …", "with Radeon Graphics"), add the
core/thread count (`… (12C/24T)`), and truncate only what still does not fit the
terminal width (`TIOCGWINSZ`, not a fixed width).

### F6: `--full` adds almost nothing
Apart from untruncated text, `--full` shows the same fields. **Fix:** in `--full`, add
fields read from cheap files:
- **GPU**: `/sys/class/drm/card*/device/{vendor,device}`, with the name resolved from
  `/usr/share/hwdata/pci.ids` or `/usr/share/misc/pci.ids` if present (stream the
  file, no full load), and otherwise the IDs.
- **Resolution(s)**: `/sys/class/drm/card*-*/modes` for connected outputs (first mode =
  current preferred; label it as "preferred" if it cannot be confirmed).
- **Battery**: `/sys/class/power_supply/BAT*` capacity and status.
- **Locale**: `LANG`.
- **Packages**: dpkg `status` count, pacman `local/` directory count, xbps `pkgdb` count.
  Read these as files, without running package managers.
- **Swap** and **Load**.

Each field is skipped silently when its source is missing.

## 3. UX

### U1: `--json`
Machine-readable output with the same fields, for nirulauncher's Health view and bug
reports. Write it with a hand-rolled serializer, because there is no serde dependency.
Escape strings correctly and test that.

### U2: Theme accent everywhere
The default layout already uses the theme colors. Use the same palette in `--classic`
(the logo in the accent color) and `--compact`. Respect 16-color terminals: fall back
from truecolor when `COLORTERM` is not `truecolor`/`24bit`.

### U3: Narrow terminals
At widths below 60 columns, `--classic` drops the logo automatically, and every mode
wraps or truncates values to the width, with no broken box characters.

Out of scope: network information, public IP, weather and images.

## 4. Gates

Automated:
- fmt, clippy, tests, `tests/smoke.sh`, and an MSRV job on 1.85 (if it is missing).
- New tests with fixture roots for `/proc`, `/sys` and the manifest (add an env override
  such as `NIRUFETCH_ROOT` for tests): F1 alignment, F2 parent walking (a fixture process
  tree), F4 manifest and theme parsing, F5 CPU name cleanup (a table of real model
  strings), each F6 source, U1 JSON validity (parse it in the test with a small checker),
  and U3 at 40/60/120 columns.
- A runtime budget test: under strace (if available in CI), a default run makes 0
  `execve` calls beyond itself.

Native: on **niruxwork**, plus othala/algiz/void if reachable (owner-authorized, read-only
use), run every flag and `--json`, and compare the values with `lscpu`, `free`, `df`,
`xrandr` and `cat /sys/class/power_supply/BAT*/capacity`. Report the timing for each host.

## 5. Release

Bump to **0.2.0** and update the README. Tag `v0.2.0` through the normal workflow and
verify the assets. Then pin it in `nirucon/nirux` `manifest`:
- If `nirux/tools/suite-pin` exists, use it.
- Otherwise bump the manifest the usual way. Change only the nirufetch line, and rebase if
  other pins land at the same time.

Install through `nirux update` and verify the version.

## 6. Final report

For each item F1–F6 and U1–U3: the change, the tests and the evidence (output samples per
mode and host, timing and execve counts). Include release URLs and remaining limits. Mark
anything unfinished as **PARTIAL** and give the exact remaining steps.
