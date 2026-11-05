mod full;

use std::{
    env, fs,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
};

/// Where system files are read from. `NIRUFETCH_ROOT` points it at a fixture
/// tree (for tests); normally it is `/`.
struct Sys {
    root: PathBuf,
}

impl Sys {
    fn from_env() -> Self {
        Self {
            root: env::var_os("NIRUFETCH_ROOT")
                .filter(|root| !root.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/")),
        }
    }

    fn path(&self, absolute: &str) -> PathBuf {
        self.root.join(absolute.trim_start_matches('/'))
    }

    fn read(&self, absolute: &str) -> String {
        read(self.path(absolute))
    }
}

#[derive(Clone, Copy)]
struct Palette {
    accent: (u8, u8, u8),
    foreground: (u8, u8, u8),
    muted: (u8, u8, u8),
    border: (u8, u8, u8),
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            accent: (0x5a, 0x5a, 0x60),
            foreground: (0xe5, 0xe5, 0xe5),
            muted: (0xa8, 0xa8, 0xa8),
            border: (0x2a, 0x2a, 0x2d),
        }
    }
}

struct Style {
    colour: bool,
    palette: Palette,
}

impl Style {
    fn paint(&self, text: &str, rgb: (u8, u8, u8), bold: bool) -> String {
        if !self.colour {
            return text.to_string();
        }
        let (r, g, b) = rgb;
        if bold {
            format!("\x1b[1;38;2;{r};{g};{b}m{text}\x1b[0m")
        } else {
            format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[0m")
        }
    }
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn os_name(sys: &Sys) -> String {
    let mut text = sys.read("/etc/os-release");
    if text.is_empty() {
        text = sys.read("/usr/lib/os-release");
    }
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
            return value.trim_matches('"').to_string();
        }
    }
    "Linux".into()
}

fn kernel() -> String {
    let mut name: libc::utsname = unsafe { std::mem::zeroed() };
    if unsafe { libc::uname(&mut name) } != 0 {
        return "unknown".into();
    }
    let release = unsafe { std::ffi::CStr::from_ptr(name.release.as_ptr()) };
    let release = release.to_string_lossy().trim().to_string();
    if release.is_empty() {
        "unknown".into()
    } else {
        release
    }
}

fn hostname(sys: &Sys) -> String {
    env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let value = sys.read("/etc/hostname").trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_else(|| "nirux".into())
}

fn username() -> String {
    env::var("USER").unwrap_or_else(|_| "user".into())
}

fn uptime_seconds(sys: &Sys) -> u64 {
    sys.read("/proc/uptime")
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0) as u64
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn memory(sys: &Sys) -> Option<(u64, u64)> {
    let text = sys.read("/proc/meminfo");
    let value = |key: &str| {
        text.lines()
            .find(|line| line.starts_with(key))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
    };
    let total = value("MemTotal:")?;
    let available = value("MemAvailable:")?;
    Some((total.saturating_sub(available) * 1024, total * 1024))
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0 / 1024.0
}

fn disk() -> Option<(u64, u64)> {
    let path = std::ffi::CString::new("/").ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let total = stat.f_blocks as u64 * stat.f_frsize as u64;
    let free = stat.f_bavail as u64 * stat.f_frsize as u64;
    Some((total.saturating_sub(free), total))
}

/// Known terminal emulators, by process name (`comm`, at most 15 bytes) or
/// executable basename, and the name shown for each.
const TERMINALS: &[(&str, &str)] = &[
    ("kitty", "kitty"),
    ("alacritty", "alacritty"),
    ("foot", "foot"),
    ("footclient", "foot"),
    ("wezterm", "wezterm"),
    ("wezterm-gui", "wezterm"),
    ("xterm", "xterm"),
    ("uxterm", "xterm"),
    ("st", "st"),
    ("st-256color", "st"),
    ("urxvt", "urxvt"),
    ("urxvtd", "urxvt"),
    ("rxvt", "rxvt"),
    ("gnome-terminal", "gnome-terminal"),
    ("gnome-terminal-", "gnome-terminal"),
    ("gnome-terminal-server", "gnome-terminal"),
    ("kgx", "gnome-console"),
    ("ptyxis", "ptyxis"),
    ("ptyxis-agent", "ptyxis"),
    ("konsole", "konsole"),
    ("xfce4-terminal", "xfce4-terminal"),
    ("mate-terminal", "mate-terminal"),
    ("lxterminal", "lxterminal"),
    ("qterminal", "qterminal"),
    ("terminator", "terminator"),
    ("tilix", "tilix"),
    ("sakura", "sakura"),
    ("terminology", "terminology"),
    ("ghostty", "ghostty"),
    ("rio", "rio"),
    ("contour", "contour"),
    ("blackbox", "blackbox"),
    ("cool-retro-term", "cool-retro-term"),
    ("tilda", "tilda"),
    ("guake", "guake"),
    ("yakuake", "yakuake"),
];

fn known_terminal(name: &str) -> Option<&'static str> {
    TERMINALS
        .iter()
        .find(|(process, _)| *process == name)
        .map(|(_, shown)| *shown)
}

/// The parent pid and `comm` of a process, from `/proc/<pid>/stat`. `comm` is
/// in parentheses and may itself contain spaces or parentheses.
fn proc_stat(sys: &Sys, pid: u32) -> Option<(u32, String)> {
    let text = sys.read(&format!("/proc/{pid}/stat"));
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let comm = text.get(open + 1..close)?.to_string();
    let ppid = text
        .get(close + 1..)?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some((ppid, comm))
}

/// Walk up from `start` to the first known terminal emulator, checking each
/// process's `comm` and then its executable. Bounded, file reads only.
fn terminal_from_parents(sys: &Sys, start: u32) -> Option<&'static str> {
    let mut pid = start;
    for _ in 0..64 {
        if pid <= 1 {
            return None;
        }
        let (ppid, comm) = proc_stat(sys, pid)?;
        if let Some(found) = known_terminal(&comm) {
            return Some(found);
        }
        if let Ok(exe) = fs::read_link(sys.path(&format!("/proc/{pid}/exe"))) {
            if let Some(found) = exe
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(known_terminal)
            {
                return Some(found);
            }
        }
        if ppid == pid {
            return None;
        }
        pid = ppid;
    }
    None
}

/// Normalise `$TERM_PROGRAM` / `$TERM` when no parent process is a terminal
/// (over SSH, inside a multiplexer, or on the console).
fn terminal_from_env(term_program: Option<&str>, term: Option<&str>) -> String {
    let clean = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    if let Some(program) = clean(term_program) {
        let lower = program.to_ascii_lowercase();
        return known_terminal(&lower)
            .map(str::to_string)
            .unwrap_or(program);
    }
    match clean(term).as_deref() {
        None | Some("dumb") => "tty".into(),
        Some("linux") => "Linux console".into(),
        Some(term) => {
            let base = term.strip_prefix("xterm-").unwrap_or(term);
            let base = base.strip_suffix("-256color").unwrap_or(base);
            known_terminal(base)
                .map(str::to_string)
                .unwrap_or_else(|| term.to_string())
        }
    }
}

fn terminal(sys: &Sys) -> String {
    let parent = unsafe { libc::getppid() };
    terminal_from_parents(sys, parent.max(0) as u32)
        .map(str::to_string)
        .unwrap_or_else(|| {
            terminal_from_env(
                env::var("TERM_PROGRAM").ok().as_deref(),
                env::var("TERM").ok().as_deref(),
            )
        })
}

struct Cpu {
    model: String,
    cores: Option<usize>,
    threads: Option<usize>,
}

impl Cpu {
    fn counts(&self) -> String {
        match (self.cores, self.threads) {
            (Some(cores), Some(threads)) => format!(" ({cores}C/{threads}T)"),
            (None, Some(threads)) => format!(" ({threads}T)"),
            _ => String::new(),
        }
    }

    /// The cleaned model name with core/thread counts; `--full` keeps the raw
    /// model string.
    fn display(&self, full: bool) -> String {
        let name = if full {
            self.model.clone()
        } else {
            short_cpu(&self.model)
        };
        format!("{name}{}", self.counts())
    }
}

/// Model name and topology from `/proc/cpuinfo`. Threads are the logical
/// processors; cores are the distinct (physical id, core id) pairs, or
/// `cpu cores` per package when those ids are missing.
fn cpu_info(sys: &Sys) -> Cpu {
    let text = sys.read("/proc/cpuinfo");
    let mut model = None;
    let mut threads = 0;
    let mut physical = String::new();
    let mut pairs = std::collections::BTreeSet::new();
    let mut packages = std::collections::BTreeSet::new();
    let mut cores_per_package = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "processor" => threads += 1,
            "model name" | "Model" | "Hardware" if model.is_none() && !value.is_empty() => {
                model = Some(value.to_string())
            }
            "physical id" => {
                physical = value.to_string();
                packages.insert(value.to_string());
            }
            "core id" => {
                pairs.insert((physical.clone(), value.to_string()));
            }
            "cpu cores" => cores_per_package = value.parse::<usize>().ok(),
            _ => {}
        }
    }
    let cores = if !pairs.is_empty() {
        Some(pairs.len())
    } else {
        cores_per_package.map(|cores| cores * packages.len().max(1))
    };
    Cpu {
        model: model.unwrap_or_else(|| "CPU".into()),
        cores: cores.filter(|&cores| cores > 0 && threads > 0),
        threads: (threads > 0).then_some(threads),
    }
}

fn basename(value: &str) -> String {
    Path::new(value)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| value.to_string())
}

/// The built-in NIRUWM theme catalog: id, display name and the colours
/// nirufetch uses (focus/accent, foreground, muted, border). A file of the same
/// id in `~/.config/niruwm/themes` takes precedence, as it does in NIRUWM.
type Rgb = (u8, u8, u8);
const BUILTIN_THEMES: &[(&str, &str, Rgb, Rgb, Rgb, Rgb)] = &[
    (
        "niru-noir",
        "Niru Noir",
        (0x5a, 0x5a, 0x60),
        (0xe5, 0xe5, 0xe5),
        (0xa8, 0xa8, 0xa8),
        (0x2a, 0x2a, 0x2d),
    ),
    (
        "niru-nord",
        "Niru Nord",
        (0x88, 0xc0, 0xd0),
        (0xec, 0xef, 0xf4),
        (0x9a, 0xa5, 0xb1),
        (0x3b, 0x42, 0x52),
    ),
    (
        "niru-satie",
        "Niru Satie",
        (0xb0, 0x8d, 0x7d),
        (0xde, 0xda, 0xd4),
        (0x8f, 0x8b, 0x86),
        (0x2a, 0x27, 0x24),
    ),
    (
        "niru-carl-larsson",
        "Niru Carl Larsson",
        (0xa6, 0x3d, 0x2f),
        (0x3b, 0x32, 0x28),
        (0x7d, 0x6f, 0x5f),
        (0xd8, 0xcd, 0xbb),
    ),
    (
        "niru-forest-vigil",
        "Niru Forest Vigil",
        (0x8a, 0xa8, 0x7e),
        (0xd8, 0xde, 0xd2),
        (0x96, 0xa2, 0x92),
        (0x2b, 0x36, 0x2c),
    ),
    (
        "niru-blood-of-urdr",
        "Niru Blood of Urdr",
        (0xc9, 0x60, 0x50),
        (0xe8, 0xd6, 0xcf),
        (0xb0, 0x9a, 0x92),
        (0x3a, 0x1d, 0x1b),
    ),
    (
        "niru-root-and-stone",
        "Niru Root and Stone",
        (0xc1, 0x97, 0x68),
        (0xea, 0xdf, 0xd2),
        (0xb5, 0xa0, 0x8c),
        (0x3a, 0x2a, 0x1e),
    ),
    (
        "niru-blue-horizon",
        "Niru Blue Horizon",
        (0x86, 0xae, 0xc8),
        (0xdc, 0xe6, 0xeb),
        (0x93, 0xa6, 0xb2),
        (0x24, 0x38, 0x46),
    ),
    (
        "niru-the-norns",
        "Niru The Norns",
        (0xc9, 0xc2, 0xb9),
        (0xec, 0xe9, 0xe4),
        (0xa6, 0xa1, 0x9b),
        (0x3b, 0x39, 0x36),
    ),
    (
        "niru-winter-nights",
        "Niru Winter Nights",
        (0xd8, 0xc4, 0xaa),
        (0xea, 0xd9, 0xc3),
        (0xb9, 0xaa, 0x9a),
        (0x4e, 0x51, 0x57),
    ),
];

struct Theme {
    id: String,
    /// The catalog name without the "Niru " prefix, e.g. "Forest Vigil".
    name: String,
    palette: Palette,
}

fn config_dir() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
}

/// A top-level `key = value` from a small TOML file: lines before the first
/// table header, with the value unquoted and a trailing comment removed.
fn toml_top_level(text: &str, wanted: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            return None;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != wanted {
            continue;
        }
        let value = value.trim();
        let value = if let Some(rest) = value.strip_prefix('"') {
            rest.split('"').next().unwrap_or_default()
        } else {
            value.split('#').next().unwrap_or_default().trim()
        };
        return Some(value.to_string());
    }
    None
}

fn hex_rgb(value: &str) -> Option<Rgb> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&value[at..at + 2], 16).ok();
    Some((byte(0)?, byte(2)?, byte(4)?))
}

fn display_theme_name(name: &str) -> String {
    name.strip_prefix("Niru ")
        .unwrap_or(name)
        .trim()
        .to_string()
}

fn theme_in(config: &Path) -> Theme {
    let id = toml_top_level(&read(config.join("config.toml")), "theme")
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "niru-noir".into());
    let mut palette = Palette::default();
    let mut name = None;
    if let Some(&(_, builtin, accent, foreground, muted, border)) =
        BUILTIN_THEMES.iter().find(|entry| entry.0 == id)
    {
        palette = Palette {
            accent,
            foreground,
            muted,
            border,
        };
        name = Some(builtin.to_string());
    }
    // A user theme file overrides the built-in one, field by field. Only plain
    // ids are looked up, so a config value can never leave the themes folder.
    if id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        let text = read(config.join("themes").join(format!("{id}.toml")));
        let colour = |key: &str| toml_top_level(&text, key).and_then(|v| hex_rgb(&v));
        if let Some(rgb) = colour("focus") {
            palette.accent = rgb;
        }
        if let Some(rgb) = colour("foreground") {
            palette.foreground = rgb;
        }
        if let Some(rgb) = colour("muted") {
            palette.muted = rgb;
        }
        if let Some(rgb) = colour("border") {
            palette.border = rgb;
        }
        if let Some(custom) = toml_top_level(&text, "name").filter(|n| !n.is_empty()) {
            name = Some(custom);
        }
    }
    Theme {
        name: display_theme_name(name.as_deref().unwrap_or(&id)),
        id,
        palette,
    }
}

fn theme() -> Theme {
    theme_in(&config_dir().join("niruwm"))
}

/// The installed NIRUX generation, as NIRUX records it: the `manifest` of
/// `~/.local/share/nirux/current` (`suite X` plus one `component X` line
/// each). `nirux --version` reports the same suite version.
#[derive(Default)]
struct Nirux {
    suite: Option<String>,
    components: Vec<(String, String)>,
}

impl Nirux {
    fn component(&self, name: &str) -> Option<&str> {
        self.components
            .iter()
            .find(|(component, _)| component == name)
            .map(|(_, version)| version.as_str())
    }
}

fn nirux_state_dir() -> PathBuf {
    env::var_os("NIRUX_PREFIX")
        .filter(|prefix| !prefix.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".local"))
        .join("share/nirux")
}

fn parse_manifest(text: &str) -> Nirux {
    let mut nirux = Nirux::default();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let (Some(key), Some(value), None) = (words.next(), words.next(), words.next()) else {
            continue;
        };
        match key {
            "schema" => {}
            "suite" => nirux.suite = Some(value.to_string()),
            _ => nirux.components.push((key.to_string(), value.to_string())),
        }
    }
    nirux
}

/// `VERSION=` near the top of the generation's `nirux` script, which is what
/// `nirux --version` prints. Only the first lines are read.
fn script_version(text: &str) -> Option<String> {
    text.lines()
        .take(20)
        .find_map(|line| line.strip_prefix("VERSION="))
        .map(|value| value.trim().trim_matches(['"', '\'']).to_string())
        .filter(|value| !value.is_empty())
}

fn nirux_in(state: &Path) -> Nirux {
    let current = state.join("current");
    let mut nirux = parse_manifest(&read(current.join("manifest")));
    if nirux.suite.is_none() {
        nirux.suite = script_version(&read_head(&current.join("bin/nirux"), 2048));
    }
    nirux
}

fn read_head(path: &Path, bytes: usize) -> String {
    use std::io::Read;
    let mut buffer = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        let _ = file.take(bytes as u64).read_to_end(&mut buffer);
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

/// NIRUWM with its installed version when this is a NIRU session (or the
/// desktop is unknown and NIRUX is installed); otherwise the desktop name.
fn window_manager(desktop: Option<&str>, nirux: &Nirux) -> String {
    let desktop = desktop.map(str::trim).filter(|d| !d.is_empty());
    let niru_session = desktop
        .map(|d| d.to_ascii_uppercase().contains("NIRU"))
        .unwrap_or(true);
    match (niru_session, nirux.component("niruwm")) {
        (true, Some(version)) => format!("NIRUWM {version}"),
        (true, None) if desktop.is_some() => "NIRUWM".into(),
        _ => desktop.unwrap_or("unknown").to_string(),
    }
}

fn logo() -> [&'static str; 6] {
    [
        "  ███╗   ██╗██╗██████╗ ██╗   ██╗",
        "  ████╗  ██║██║██╔══██╗██║   ██║",
        "  ██╔██╗ ██║██║██████╔╝██║   ██║",
        "  ██║╚██╗██║██║██╔══██╗██║   ██║",
        "  ██║ ╚████║██║██║  ██║╚██████╔╝",
        "  ╚═╝  ╚═══╝╚═╝╚═╝  ╚═╝ ╚═════╝",
    ]
}

/// The terminal width from `TIOCGWINSZ` on stdout, else a positive
/// `$COLUMNS`, else unknown.
fn terminal_width() -> Option<usize> {
    if io::stdout().is_terminal() {
        let mut winsize: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) } == 0
            && winsize.ws_col > 0
        {
            return Some(winsize.ws_col as usize);
        }
    }
    env::var("COLUMNS")
        .ok()
        .and_then(|columns| columns.trim().parse::<usize>().ok())
        .filter(|&columns| columns > 0)
}

fn fit(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".into();
    }
    value.chars().take(width - 1).collect::<String>() + "…"
}

/// Strip vendor noise from a CPU model: trademark marks, "CPU", the clock
/// ("@ 3.30GHz"), integrated graphics ("w/ Radeon 890M", "with Radeon
/// Graphics") and "N-Core Processor".
fn short_cpu(value: &str) -> String {
    let mut v = value.to_string();
    for mark in ["(R)", "(r)", "(TM)", "(tm)", "®", "™"] {
        v = v.replace(mark, "");
    }
    for cut in [" @ ", " w/ ", " with "] {
        if let Some(at) = v.to_ascii_lowercase().find(cut) {
            v.truncate(at);
        }
    }
    let short = v
        .split_whitespace()
        .filter(|word| {
            !matches!(*word, "CPU" | "Processor") && !word.to_ascii_lowercase().ends_with("-core")
        })
        .collect::<Vec<_>>()
        .join(" ");
    if short.is_empty() {
        value.trim().to_string()
    } else {
        short
    }
}

#[derive(Clone, Copy, Default)]
struct Layout {
    compact: bool,
    full: bool,
    plain: bool,
}

/// The default (and `--compact`/`--full`) layout. The header and rules stay
/// within 44 columns; values use the rest of the terminal width and are cut
/// only when they still do not fit. `width` is `None` when it is unknown (not
/// a terminal), and then nothing is cut.
fn default_lines(
    title: &str,
    info: &[(String, String)],
    style: &Style,
    layout: Layout,
    width: Option<usize>,
) -> Vec<String> {
    let mut out = Vec::new();
    let available = width.unwrap_or(usize::MAX).max(1);
    let rule_width = if layout.full {
        width.unwrap_or(44).max(1)
    } else {
        available.min(44)
    };
    let label_width = if layout.compact || rule_width < 30 {
        5
    } else {
        8
    };
    let gap = 2;
    let value_width = available.saturating_sub(label_width + gap);
    let line = "─".repeat(rule_width);
    out.push(style.paint(&fit("NIRUX", rule_width), style.palette.accent, true));
    if !layout.compact && rule_width >= 22 {
        out.push(style.paint(
            &fit("A minimal X11 desktop", rule_width),
            style.palette.muted,
            false,
        ));
    }
    out.push(style.paint(&line, style.palette.border, false));
    out.push(String::new());
    out.push(style.paint(&fit(title, available), style.palette.accent, true));
    out.push(String::new());
    for (index, (key, value)) in info.iter().enumerate() {
        if key == "Memory" && index > 0 {
            out.push(String::new());
        }
        let label = match key.as_str() {
            "Terminal" if label_width == 5 => "Term",
            "Kernel" if label_width == 5 => "Kern",
            "Memory" if label_width == 5 => "Mem",
            "Uptime" if label_width == 5 => "Up",
            "Display" if label_width == 5 => "Disp",
            "Battery" if label_width == 5 => "Bat",
            "Packages" if label_width == 5 => "Pkgs",
            "Locale" if label_width == 5 => "Lang",
            "Extras" if label_width == 5 => "Extra",
            other => other,
        };
        let display = match key.as_str() {
            "OS" if !layout.full => value.replace("Debian GNU/Linux", "Debian"),
            _ => value.clone(),
        };
        if available <= label_width + gap {
            out.push(style.paint(
                &fit(&format!("{label} {display}"), available),
                style.palette.foreground,
                false,
            ));
            continue;
        }
        let display = if layout.full {
            display
        } else {
            fit(&display, value_width)
        };
        out.push(format!(
            "{}  {}",
            style.paint(
                &format!("{label:<label_width$}"),
                style.palette.muted,
                false
            ),
            style.paint(&display, style.palette.foreground, false)
        ));
    }
    if !layout.plain && !layout.compact && rule_width >= 20 {
        out.push(String::new());
        out.push(style.paint(&line, style.palette.border, false));
    }
    out
}

/// Everything nirufetch shows. The `extra` part is collected only for
/// `--full` and `--json`.
struct Info {
    user: String,
    host: String,
    os: String,
    nirux: Nirux,
    kernel: String,
    uptime: u64,
    shell: String,
    wm: String,
    terminal: String,
    cpu: Cpu,
    memory: Option<(u64, u64)>,
    disk: Option<(u64, u64)>,
    theme: Theme,
    extra: Option<Extra>,
}

struct Extra {
    gpus: Vec<full::Gpu>,
    outputs: Vec<full::Output>,
    batteries: Vec<full::Battery>,
    locale: Option<String>,
    packages: Vec<(&'static str, usize)>,
    swap: Option<(u64, u64)>,
    load: Option<[f64; 3]>,
    extras: Vec<(String, Option<String>)>,
}

fn locale() -> Option<String> {
    ["LANG", "LC_ALL"]
        .iter()
        .filter_map(|key| env::var(key).ok())
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn collect(sys: &Sys, theme: Theme, with_extra: bool) -> Info {
    let state = nirux_state_dir();
    let nirux = nirux_in(&state);
    let extra = with_extra.then(|| Extra {
        gpus: full::gpus(sys),
        outputs: full::outputs(sys),
        batteries: full::batteries(sys),
        locale: locale(),
        packages: full::packages(sys),
        swap: full::swap(sys),
        load: full::load(sys),
        extras: full::extras(&state),
    });
    Info {
        user: username(),
        host: hostname(sys),
        os: os_name(sys),
        wm: window_manager(env::var("XDG_CURRENT_DESKTOP").ok().as_deref(), &nirux),
        nirux,
        kernel: kernel(),
        uptime: uptime_seconds(sys),
        shell: basename(&env::var("SHELL").unwrap_or_else(|_| "sh".into())),
        terminal: terminal(sys),
        cpu: cpu_info(sys),
        memory: memory(sys),
        disk: disk(),
        theme,
        extra,
    }
}

fn usage(used: u64, total: u64, decimals: usize) -> String {
    let percent = if total == 0 {
        0.0
    } else {
        used as f64 / total as f64 * 100.0
    };
    format!(
        "{:.decimals$} GiB / {:.decimals$} GiB ({percent:.0}%)",
        gib(used),
        gib(total)
    )
}

/// The labelled rows of the text layouts, in order. Empty values are dropped.
fn rows(info: &Info, full: bool) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut push = |label: &str, value: String| rows.push((label.to_string(), value));
    push("OS", info.os.clone());
    push("NIRUX", info.nirux.suite.clone().unwrap_or_default());
    push("Kernel", info.kernel.clone());
    push("Uptime", format_uptime(info.uptime));
    push("Shell", info.shell.clone());
    push("WM", info.wm.clone());
    push("Terminal", info.terminal.clone());
    push("CPU", info.cpu.display(full));
    let extra = info.extra.as_ref().filter(|_| full);
    if let Some(extra) = extra {
        for gpu in &extra.gpus {
            push("GPU", gpu.display());
        }
    }
    push(
        "Memory",
        info.memory
            .map(|(used, total)| usage(used, total, 1))
            .unwrap_or_else(|| "unknown".into()),
    );
    if let Some(extra) = extra {
        if let Some((used, total)) = extra.swap {
            push(
                "Swap",
                if total == 0 {
                    "none".into()
                } else {
                    usage(used, total, 1)
                },
            );
        }
    }
    push(
        "Disk",
        info.disk
            .map(|(used, total)| usage(used, total, 0))
            .unwrap_or_else(|| "unknown".into()),
    );
    if let Some(extra) = extra {
        for output in &extra.outputs {
            push("Display", output.display());
        }
        for battery in &extra.batteries {
            push("Battery", battery.display());
        }
        push("Packages", full::packages_display(&extra.packages));
        push("Locale", extra.locale.clone().unwrap_or_default());
        if let Some([one, five, fifteen]) = extra.load {
            push("Load", format!("{one:.2} {five:.2} {fifteen:.2}"));
        }
        push("Extras", full::extras_display(&extra.extras));
    }
    push(
        "Theme",
        if full {
            format!("{} ({})", info.theme.name, info.theme.id)
        } else {
            info.theme.name.clone()
        },
    );
    rows.retain(|(_, value)| !value.is_empty());
    rows
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("NIRUFETCH {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|arg| arg == "--help") {
        println!(
            "Usage: nirufetch [--classic] [--compact] [--full] [--no-color] [--no-logo] [--plain]"
        );
        println!("NIRU terminal system fetch. Reads local files; no network or daemon.");
        return;
    }
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    let full = flag("--full");
    let classic = flag("--classic");
    let theme = theme();
    let style = Style {
        colour: io::stdout().is_terminal()
            && env::var_os("NO_COLOR").is_none()
            && !flag("--no-color")
            && !flag("--plain")
            && env::var("TERM").map(|term| term != "dumb").unwrap_or(true),
        palette: theme.palette,
    };
    let sys = Sys::from_env();
    let info = collect(&sys, theme, full);
    let title = format!("{}@{}", info.user, info.host);
    let rows = rows(&info, full);
    let lines = if classic {
        classic_lines(
            &title,
            &rows,
            &style,
            !flag("--no-logo") && !flag("--plain"),
        )
    } else {
        let layout = Layout {
            compact: flag("--compact"),
            full,
            plain: flag("--plain"),
        };
        default_lines(&title, &rows, &style, layout, terminal_width())
    };
    for line in lines {
        println!("{line}");
    }
}

fn classic_lines(
    title: &str,
    info: &[(String, String)],
    style: &Style,
    show_logo: bool,
) -> Vec<String> {
    let mut out = vec![
        style.paint(title, style.palette.accent, true),
        style.paint(
            &"─".repeat(title.chars().count()),
            style.palette.border,
            false,
        ),
        String::new(),
    ];
    let logo = logo();
    let logo_width = logo
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let label_width = info
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    let rows = if show_logo {
        logo.len().max(info.len())
    } else {
        info.len()
    };
    for index in 0..rows {
        let left = if show_logo && index < logo.len() {
            let padded = format!("{:<width$}", logo[index], width = logo_width);
            style.paint(&padded, style.palette.accent, true)
        } else {
            " ".repeat(logo_width)
        };
        let right = info.get(index).map(|(label, value)| {
            format!(
                "{} {} {}",
                style.paint(
                    &format!("{label:<label_width$}"),
                    style.palette.muted,
                    false
                ),
                style.paint("│", style.palette.border, false),
                style.paint(value, style.palette.foreground, false)
            )
        });
        out.push(match right {
            Some(right) if show_logo => format!("{left}  {right}"),
            Some(right) => right,
            None => left,
        });
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn uptime_formats_units() {
        assert_eq!(format_uptime(59), "0m");
        assert_eq!(format_uptime(3_600), "1h 0m");
        assert_eq!(format_uptime(90_000), "1d 1h 0m");
    }

    #[test]
    fn kernel_comes_from_uname_without_a_process() {
        let release = read("/proc/sys/kernel/osrelease");
        if !release.trim().is_empty() {
            assert_eq!(kernel(), release.trim());
        }
    }

    #[test]
    fn basename_handles_paths() {
        assert_eq!(basename("/usr/bin/fish"), "fish");
        assert_eq!(basename("fish"), "fish");
    }

    #[test]
    fn fit_uses_cell_ellipsis() {
        assert_eq!(fit("abc", 3), "abc");
        assert_eq!(fit("abcd", 3), "ab…");
        assert_eq!(fit("abcd", 1), "…");
    }

    #[test]
    fn cpu_shortening_removes_vendor_noise() {
        for (raw, short) in [
            (
                "11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz",
                "11th Gen Intel Core i7-1165G7",
            ),
            ("Intel(R) Xeon(R) W-1250 CPU @ 3.30GHz", "Intel Xeon W-1250"),
            (
                "AMD Ryzen AI 9 HX PRO 375 w/ Radeon 890M",
                "AMD Ryzen AI 9 HX PRO 375",
            ),
            (
                "AMD Ryzen 7 5800H with Radeon Graphics",
                "AMD Ryzen 7 5800H",
            ),
            ("AMD Ryzen 9 7950X 16-Core Processor", "AMD Ryzen 9 7950X"),
            ("AMD FX(tm)-8350 Eight-Core Processor", "AMD FX-8350"),
            ("AMD EPYC 7763 64-Core Processor", "AMD EPYC 7763"),
            ("Intel(R) Core(TM) Ultra 7 155H", "Intel Core Ultra 7 155H"),
            (
                "Intel(R) Core(TM)2 Duo CPU     T7700  @ 2.40GHz",
                "Intel Core2 Duo T7700",
            ),
            (
                "Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz",
                "Intel Xeon E5-2680 v4",
            ),
            (
                "12th Gen Intel(R) Core(TM) i7-1260P",
                "12th Gen Intel Core i7-1260P",
            ),
            (
                "Intel(R) Celeron(R) N4020 CPU @ 1.10GHz",
                "Intel Celeron N4020",
            ),
            ("Apple M2", "Apple M2"),
        ] {
            assert_eq!(short_cpu(raw), short, "{raw}");
        }
    }

    #[test]
    fn cpu_counts_cores_and_threads() {
        let fixture = Fixture::new("cpuinfo");
        let mut text = String::new();
        // 1 package, 2 cores, 2 threads each.
        for (processor, core) in [(0, 0), (1, 1), (2, 0), (3, 1)] {
            text.push_str(&format!(
                "processor\t: {processor}\nmodel name\t: AMD Ryzen AI 9 HX PRO 375 w/ Radeon 890M\nphysical id\t: 0\ncore id\t\t: {core}\ncpu cores\t: 2\n\n"
            ));
        }
        fixture.write("/proc/cpuinfo", &text);
        let cpu = cpu_info(&fixture.sys());
        assert_eq!((cpu.cores, cpu.threads), (Some(2), Some(4)));
        assert_eq!(cpu.display(false), "AMD Ryzen AI 9 HX PRO 375 (2C/4T)");
        assert_eq!(
            cpu.display(true),
            "AMD Ryzen AI 9 HX PRO 375 w/ Radeon 890M (2C/4T)"
        );
        // ARM-style cpuinfo without topology ids.
        fixture.write(
            "/proc/cpuinfo",
            "processor\t: 0\nBogoMIPS\t: 1\n\nprocessor\t: 1\n\nModel\t: Raspberry Pi 4 Model B\n",
        );
        let cpu = cpu_info(&fixture.sys());
        assert_eq!(cpu.display(false), "Raspberry Pi 4 Model B (2T)");
        // Missing file.
        let empty = Fixture::new("cpuinfo-empty");
        assert_eq!(cpu_info(&empty.sys()).display(false), "CPU");
    }

    #[test]
    fn values_use_the_terminal_width_before_cutting() {
        let style = Style {
            colour: false,
            palette: Palette::default(),
        };
        let cpu = "AMD Ryzen AI 9 HX PRO 375 (12C/24T)".to_string();
        let info = vec![("CPU".to_string(), cpu.clone())];
        let layout = Layout::default();
        let wide = default_lines("u@h", &info, &style, layout, Some(80));
        assert!(wide.iter().any(|line| line.ends_with(&cpu)), "{wide:?}");
        assert!(wide.iter().all(|line| line.chars().count() <= 80));
        let narrow = default_lines("u@h", &info, &style, layout, Some(30));
        assert!(narrow.iter().all(|line| line.chars().count() <= 30));
        assert!(narrow.iter().any(|line| line.ends_with('…')));
        let unknown = default_lines("u@h", &info, &style, layout, None);
        assert!(unknown.iter().any(|line| line.ends_with(&cpu)));
    }

    /// A throw-away fixture root under the temp dir, removed on drop.
    pub(crate) struct Fixture {
        pub root: PathBuf,
    }

    impl Fixture {
        pub fn new(name: &str) -> Self {
            let root =
                env::temp_dir().join(format!("nirufetch-test-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        pub fn write(&self, path: &str, text: &str) {
            let path = self.root.join(path.trim_start_matches('/'));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        pub fn sys(&self) -> Sys {
            Sys {
                root: self.root.clone(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn process(fixture: &Fixture, pid: u32, comm: &str, ppid: u32) {
        fixture.write(
            &format!("/proc/{pid}/stat"),
            &format!("{pid} ({comm}) S {ppid} {pid} {pid} 34816 0 0 0\n"),
        );
    }

    #[test]
    fn terminal_walks_parents_to_the_first_emulator() {
        let fixture = Fixture::new("terminal-walk");
        // init → kitty → fish → tmux client (odd comm) → our shell
        process(&fixture, 1, "systemd", 0);
        process(&fixture, 40, "kitty", 1);
        process(&fixture, 41, "fish", 40);
        process(&fixture, 42, "weird (name) x", 41);
        process(&fixture, 43, "bash", 42);
        assert_eq!(terminal_from_parents(&fixture.sys(), 43), Some("kitty"));
        // gnome-terminal-server is truncated to 15 bytes in comm.
        process(&fixture, 50, "gnome-terminal-", 1);
        process(&fixture, 51, "bash", 50);
        assert_eq!(
            terminal_from_parents(&fixture.sys(), 51),
            Some("gnome-terminal")
        );
        // No emulator in the chain (an SSH login).
        process(&fixture, 60, "sshd", 1);
        process(&fixture, 61, "fish", 60);
        assert_eq!(terminal_from_parents(&fixture.sys(), 61), None);
        // A loop or a missing process ends the walk.
        process(&fixture, 70, "bash", 70);
        assert_eq!(terminal_from_parents(&fixture.sys(), 70), None);
        assert_eq!(terminal_from_parents(&fixture.sys(), 999), None);
    }

    #[test]
    fn terminal_env_fallback_normalises_names() {
        assert_eq!(terminal_from_env(Some("WezTerm"), Some("x")), "wezterm");
        assert_eq!(terminal_from_env(Some("vscode"), None), "vscode");
        assert_eq!(terminal_from_env(None, Some("xterm-kitty")), "kitty");
        assert_eq!(terminal_from_env(None, Some("foot")), "foot");
        assert_eq!(terminal_from_env(None, Some("st-256color")), "st");
        assert_eq!(terminal_from_env(None, Some("dumb")), "tty");
        assert_eq!(terminal_from_env(None, None), "tty");
        assert_eq!(terminal_from_env(None, Some("linux")), "Linux console");
        assert_eq!(terminal_from_env(None, Some("screen")), "screen");
    }

    #[test]
    fn manifest_gives_suite_and_component_versions() {
        let fixture = Fixture::new("manifest");
        fixture.write(
            "current/manifest",
            "schema 1\nsuite 0.12.205\nniruwm 0.16.172\nnirubar 0.18.84\nbroken line here\n",
        );
        let nirux = nirux_in(&fixture.root);
        assert_eq!(nirux.suite.as_deref(), Some("0.12.205"));
        assert_eq!(nirux.component("niruwm"), Some("0.16.172"));
        assert_eq!(nirux.component("nirufetch"), None);
        assert_eq!(window_manager(Some("NIRU"), &nirux), "NIRUWM 0.16.172");
        assert_eq!(window_manager(None, &nirux), "NIRUWM 0.16.172");
        assert_eq!(window_manager(Some("GNOME"), &nirux), "GNOME");
        let none = Nirux::default();
        assert_eq!(window_manager(Some("NIRU"), &none), "NIRUWM");
        assert_eq!(window_manager(None, &none), "unknown");
        // Without a suite line, fall back to the script's VERSION= line.
        fixture.write("current/manifest", "schema 1\nniruwm 0.16.172\n");
        fixture.write(
            "current/bin/nirux",
            "#!/bin/sh\nset -eu\n\nVERSION=0.12.205\nPREFIX=x\n",
        );
        assert_eq!(nirux_in(&fixture.root).suite.as_deref(), Some("0.12.205"));
        // Nothing installed: no NIRUX row.
        let empty = Fixture::new("manifest-empty");
        assert_eq!(nirux_in(&empty.root).suite, None);
    }

    #[test]
    fn theme_uses_catalog_names_and_user_files() {
        let fixture = Fixture::new("theme");
        let config = fixture.root.join("niruwm");
        // Default when nothing is configured.
        let theme = theme_in(&config);
        assert_eq!(
            (theme.id.as_str(), theme.name.as_str()),
            ("niru-noir", "Noir")
        );
        // A built-in theme, with a comment and a later table using `theme`.
        fixture.write(
            "niruwm/config.toml",
            "# x\ntheme = \"niru-forest-vigil\" # mine\n[other]\ntheme = \"nope\"\n",
        );
        let theme = theme_in(&config);
        assert_eq!(theme.id, "niru-forest-vigil");
        assert_eq!(theme.name, "Forest Vigil");
        assert_eq!(theme.palette.accent, (0x8a, 0xa8, 0x7e));
        // A user file overrides the built-in palette and name.
        fixture.write(
            "niruwm/themes/niru-forest-vigil.toml",
            "name = \"Niru Forest Night\"\nfocus = \"#102030\"\nmuted = \"bad\"\n",
        );
        let theme = theme_in(&config);
        assert_eq!(theme.name, "Forest Night");
        assert_eq!(theme.palette.accent, (0x10, 0x20, 0x30));
        assert_eq!(theme.palette.muted, (0x96, 0xa2, 0x92));
        // A custom theme with its own file.
        fixture.write("niruwm/config.toml", "theme = \"my-own\"\n");
        fixture.write(
            "niruwm/themes/my-own.toml",
            "name = \"Harbour\"\nforeground = \"aabbcc\"\n",
        );
        let theme = theme_in(&config);
        assert_eq!(
            (theme.id.as_str(), theme.name.as_str()),
            ("my-own", "Harbour")
        );
        assert_eq!(theme.palette.foreground, (0xaa, 0xbb, 0xcc));
        // A path-like id is never looked up as a file.
        fixture.write("niruwm/config.toml", "theme = \"../../etc\"\n");
        assert_eq!(theme_in(&config).name, "../../etc");
    }

    fn separator_columns(lines: &[String]) -> Vec<usize> {
        lines
            .iter()
            .filter_map(|line| line.chars().position(|c| c == '│'))
            .collect()
    }

    #[test]
    fn classic_separators_form_a_straight_column() {
        let style = Style {
            colour: false,
            palette: Palette::default(),
        };
        let info: Vec<(String, String)> = [
            ("OS", "Debian"),
            ("Kernel", "6.12"),
            ("Terminal", "kitty"),
            ("WM", "NIRUWM"),
        ]
        .iter()
        .map(|(l, v)| (l.to_string(), v.to_string()))
        .collect();
        for show_logo in [true, false] {
            let columns = separator_columns(&classic_lines("u@h", &info, &style, show_logo));
            assert_eq!(columns.len(), info.len());
            assert!(
                columns.windows(2).all(|pair| pair[0] == pair[1]),
                "{columns:?}"
            );
        }
    }
}
