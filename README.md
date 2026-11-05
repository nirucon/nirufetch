# NIRUFETCH

**Part of [NIRUX - A minimal X11 desktop by Nicklas Rudolfsson](https://github.com/nirucon/nirux).**

**Early beta.** A fast, local terminal system fetch in the NIRUX visual style.
It reads `/proc`, `/sys`, `/etc`, the NIRUX generation manifest and the NIRUWM
theme; no network, daemon or background service, and it starts no other
process. A run takes a few milliseconds.

## Install

NIRUX installs it automatically. Standalone:

```sh
git clone https://github.com/nirucon/nirufetch
cd nirufetch
./install.sh
./install.sh --build
```

## Use

```sh
nirufetch
nirufetch --plain
nirufetch --no-logo
nirufetch --compact
nirufetch --classic
nirufetch --full
nirufetch --json
```

`--plain` and `--no-color` are also honoured through `NO_COLOR`, and colour is
off when stdout is not a terminal. The theme follows `theme` in
`~/.config/niruwm/config.toml` (Noir, Nord, Satie, Carl Larsson, Forest Vigil,
Blood of Urdr, Root and Stone, Blue Horizon, The Norns, Winter Nights, or your
own `~/.config/niruwm/themes/NAME.toml`, whose name and colours are used).
Truecolor is used when `COLORTERM` is `truecolor`/`24bit` or the terminal is
known to support it; otherwise the palette falls back to 256 or 16 colours.

## Fields

- **NIRUX** and **WM** (`NIRUWM 0.16.172`) come from the installed generation's
  `~/.local/share/nirux/current/manifest` (or `$NIRUX_PREFIX/share/nirux`), the
  same versions `nirux --version` and `nirux update` report.
- **Terminal** is found by walking the parent processes up to the first known
  terminal emulator (kitty, alacritty, foot, wezterm, xterm, st, urxvt,
  gnome-terminal, konsole, ghostty and more), then `$TERM_PROGRAM`, then
  `$TERM`.
- **CPU** drops vendor noise ("(R)", "CPU @ 3.30GHz", "w/ Radeon 890M") and
  adds the core/thread count (`(6C/12T)`).
- `--full` shows untrimmed values (wrapped to the terminal) and adds GPU (names
  from `pci.ids` when installed), connected displays with their preferred mode,
  battery, swap, packages (dpkg, pacman and xbps databases, read as files),
  locale, load, the selected NIRUX extras with the versions the installed
  generation pins, and the theme id. Fields whose source is missing are
  skipped.
- `--json` prints all of that as JSON, for scripts, the launcher's Health view
  and bug reports.

## Build

```sh
cargo test --locked
cargo build --release --locked
sh tests/smoke.sh target/release/nirufetch
```

Rust 1.85 or newer; the only dependency is `libc`. `NIRUFETCH_ROOT` points
the `/proc`, `/sys`, `/etc` and `/var` reads at another tree (for tests).

MIT. Copyright 2026 Nicklas Rudolfsson.

## Layout

The default layout is a narrow, aligned NIRUX system summary (a 44-column header) that adapts to tiled terminals: values use the real terminal width and are cut only when they still do not fit. Use `--compact` for smaller windows, `--classic` for the original large ASCII logo (dropped automatically below 60 columns), and `--full` to wrap values instead of cutting them. The default layout displays the NIRUX identity and the tagline “A minimal X11 desktop”.
