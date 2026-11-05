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

fn os_name() -> String {
    for line in read("/etc/os-release").lines() {
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

fn hostname() -> String {
    env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let value = read("/etc/hostname").trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_else(|| "nirux".into())
}

fn username() -> String {
    env::var("USER").unwrap_or_else(|_| "user".into())
}

fn uptime_seconds() -> u64 {
    read("/proc/uptime")
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

fn memory() -> Option<(u64, u64)> {
    let text = read("/proc/meminfo");
    let value = |key: &str| {
        text.lines()
            .find(|line| line.starts_with(key))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
    };
    let total = value("MemTotal:")?;
    let available = value("MemAvailable:")?;
    Some(((total - available) * 1024, total * 1024))
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
    Some((total - free, total))
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

fn cpu_model() -> String {
    let text = read("/proc/cpuinfo");
    text.lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == "model name").then(|| value.trim().to_string())
        })
        .unwrap_or_else(|| "CPU".into())
}

fn component(value: Option<String>, fallback: &str) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.into())
}

fn basename(value: &str) -> String {
    Path::new(value)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| value.to_string())
}

fn theme() -> (String, Palette) {
    let path = env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config")
        })
        .join("niruwm/config.toml");
    let name = read(path)
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "theme").then(|| value.trim().trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "niru-noir".into());
    let mut palette = Palette::default();
    match name.as_str() {
        "niru-nord" => {
            palette.accent = (0x88, 0xc0, 0xd0);
            palette.foreground = (0xec, 0xef, 0xf4);
            palette.muted = (0x9a, 0xa5, 0xb1);
            palette.border = (0x3b, 0x42, 0x52);
        }
        "niru-satie" => {
            palette.accent = (0xb0, 0x8d, 0x7d);
            palette.foreground = (0xde, 0xda, 0xd4);
            palette.muted = (0x8f, 0x8b, 0x86);
            palette.border = (0x2a, 0x27, 0x24);
        }
        "niru-carl-larsson" => {
            palette.accent = (0xa6, 0x3d, 0x2f);
            palette.foreground = (0x3b, 0x32, 0x28);
            palette.muted = (0x7d, 0x6f, 0x5f);
            palette.border = (0xd8, 0xcd, 0xbb);
        }
        "niru-forest-vigil" => {
            palette.accent = (0x8a, 0xa8, 0x7e);
            palette.foreground = (0xd8, 0xde, 0xd2);
            palette.muted = (0x96, 0xa2, 0x92);
            palette.border = (0x2b, 0x36, 0x2c);
        }
        "niru-blood-of-urdr" => {
            palette.accent = (0xc9, 0x60, 0x50);
            palette.foreground = (0xe8, 0xd6, 0xcf);
            palette.muted = (0xb0, 0x9a, 0x92);
            palette.border = (0x3a, 0x1d, 0x1b);
        }
        "niru-root-and-stone" => {
            palette.accent = (0xc1, 0x97, 0x68);
            palette.foreground = (0xea, 0xdf, 0xd2);
            palette.muted = (0xb5, 0xa0, 0x8c);
            palette.border = (0x3a, 0x2a, 0x1e);
        }
        "niru-blue-horizon" => {
            palette.accent = (0x86, 0xae, 0xc8);
            palette.foreground = (0xdc, 0xe6, 0xeb);
            palette.muted = (0x93, 0xa6, 0xb2);
            palette.border = (0x24, 0x38, 0x46);
        }
        "niru-the-norns" => {
            palette.accent = (0xc9, 0xc2, 0xb9);
            palette.foreground = (0xec, 0xe9, 0xe4);
            palette.muted = (0xa6, 0xa1, 0x9b);
            palette.border = (0x3b, 0x39, 0x36);
        }
        "niru-winter-nights" => {
            palette.accent = (0xd8, 0xc4, 0xaa);
            palette.foreground = (0xea, 0xd9, 0xc3);
            palette.muted = (0xb9, 0xaa, 0x9a);
            palette.border = (0x4e, 0x51, 0x57);
        }
        _ => {}
    }
    (name, palette)
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

fn terminal_width() -> usize {
    if !io::stdout().is_terminal() {
        return 44;
    }
    let mut winsize: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) } == 0
        && winsize.ws_col > 0
    {
        winsize.ws_col as usize
    } else {
        44
    }
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

fn short_cpu(value: &str) -> String {
    let mut v = value
        .replace("(R)", "")
        .replace("(TM)", "")
        .replace("CPU", "");
    if let Some((head, _)) = v.split_once(" @ ") {
        v = head.to_string();
    }
    v.split_whitespace().collect::<Vec<_>>().join(" ")
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
    let style = Style {
        colour: io::stdout().is_terminal()
            && env::var_os("NO_COLOR").is_none()
            && !args
                .iter()
                .any(|arg| arg == "--no-color" || arg == "--plain")
            && env::var("TERM").map(|term| term != "dumb").unwrap_or(true),
        palette: theme().1,
    };
    let classic = args.iter().any(|arg| arg == "--classic");
    let show_logo = classic
        && !args
            .iter()
            .any(|arg| arg == "--no-logo" || arg == "--plain");
    let (theme_name, _) = theme();
    let sys = Sys::from_env();
    let mem = memory();
    let disk = disk();
    let mut info = vec![
        ("OS".to_string(), os_name()),
        ("Kernel".to_string(), kernel()),
        ("Uptime".to_string(), format_uptime(uptime_seconds())),
        (
            "Shell".to_string(),
            basename(&env::var("SHELL").unwrap_or_else(|_| "sh".into())),
        ),
        (
            "WM".to_string(),
            component(env::var("XDG_CURRENT_DESKTOP").ok(), "unknown"),
        ),
        ("Terminal".to_string(), terminal(&sys)),
        ("CPU".to_string(), cpu_model()),
        (
            "Memory".to_string(),
            mem.map(|(used, total)| {
                format!(
                    "{:.1} GiB / {:.1} GiB ({:.0}%)",
                    gib(used),
                    gib(total),
                    used as f64 / total as f64 * 100.0
                )
            })
            .unwrap_or_else(|| "unknown".into()),
        ),
        (
            "Disk".to_string(),
            disk.map(|(used, total)| {
                format!(
                    "{:.0} GiB / {:.0} GiB ({:.0}%)",
                    gib(used),
                    gib(total),
                    used as f64 / total as f64 * 100.0
                )
            })
            .unwrap_or_else(|| "unknown".into()),
        ),
        ("Theme".to_string(), theme_name),
    ];
    info.retain(|(_, value)| !value.is_empty());

    if !classic {
        let plain = args.iter().any(|arg| arg == "--plain");
        let full = args.iter().any(|arg| arg == "--full");
        let compact = args.iter().any(|arg| arg == "--compact");
        let width = if full {
            terminal_width()
        } else {
            terminal_width().min(44)
        };
        let width = width.max(1);
        let label_width = if compact || width < 30 { 5 } else { 8 };
        let gap = 2;
        let value_width = width.saturating_sub(label_width + gap);
        let line = "─".repeat(width);
        println!(
            "{}",
            style.paint(&fit("NIRUX", width), style.palette.accent, true)
        );
        if !compact && width >= 22 {
            println!(
                "{}",
                style.paint(
                    &fit("A minimal X11 desktop", width),
                    style.palette.muted,
                    false
                )
            );
        }
        println!("{}", style.paint(&line, style.palette.border, false));
        println!();
        println!(
            "{}",
            style.paint(
                &fit(&format!("{}@{}", username(), hostname()), width),
                style.palette.accent,
                true
            )
        );
        println!();
        for (index, (label, value)) in info.iter().enumerate() {
            if index == 7 {
                println!();
            }
            let label = match label.as_str() {
                "Terminal" if label_width == 5 => "Term",
                "Kernel" if label_width == 5 => "Kern",
                "Memory" if label_width == 5 => "Mem",
                "Uptime" if label_width == 5 => "Up",
                "Theme" if label_width == 5 => "Theme",
                other => other,
            };
            let display = match info[index].0.as_str() {
                "OS" if !full => value.replace("Debian GNU/Linux", "Debian"),
                "CPU" if !full => short_cpu(value),
                _ => value.clone(),
            };
            if width <= label_width + gap {
                println!(
                    "{}",
                    style.paint(
                        &fit(&format!("{label} {display}"), width),
                        style.palette.foreground,
                        false
                    )
                );
                continue;
            }
            let display = if full {
                display
            } else {
                fit(&display, value_width)
            };
            println!(
                "{}  {}",
                style.paint(
                    &format!("{label:<label_width$}"),
                    style.palette.muted,
                    false
                ),
                style.paint(&display, style.palette.foreground, false)
            );
        }
        if !plain && !compact && width >= 20 {
            println!();
            println!("{}", style.paint(&line, style.palette.border, false));
        }
        return;
    }

    for line in classic_lines(
        &format!("{}@{}", username(), hostname()),
        &info,
        &style,
        show_logo,
    ) {
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
mod tests {
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
    fn cpu_shortening_removes_trademark_and_frequency() {
        let short = short_cpu("11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz");
        assert_eq!(short, "11th Gen Intel Core i7-1165G7");
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
