# NIRUFETCH

**Part of [NIRUX - A minimal X11 desktop by Nicklas Rudolfsson](https://github.com/nirucon/nirux).**

**Early beta.** A fast, local terminal system fetch in the NIRU visual style.
It reads `/proc`, `/etc` and the NIRUWM theme; no network, daemon or background
service.

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
```

`--plain` and `--no-color` are also honoured through `NO_COLOR`. The theme
follows `theme` in `~/.config/niruwm/config.toml` (Niru Noir, Nord, Satie,
Carl Larsson).

## Build

```sh
cargo test --locked
cargo build --release --locked
```

MIT. Copyright 2026 Nicklas Rudolfsson.

## Layout

The default layout is a narrow, aligned NIRUX system summary (up to 44 terminal columns) that adapts to tiled terminals. Use `--compact` for smaller windows, `--classic` for the original large ASCII logo, and `--full` to avoid value truncation. The default layout displays the NIRUX identity and the tagline “A minimal X11 desktop”.
