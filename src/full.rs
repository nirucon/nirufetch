//! The extra `--full` (and `--json`) fields. Every source is a cheap file
//! read; a field whose source is missing is skipped silently.

use std::{fs, path::Path};

use crate::{read, Sys};

#[derive(Clone, Debug, PartialEq)]
pub struct Gpu {
    pub card: String,
    pub vendor_id: u16,
    pub device_id: u16,
    /// Resolved from pci.ids, when it is installed.
    pub name: Option<String>,
}

impl Gpu {
    pub fn display(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("{:04x}:{:04x}", self.vendor_id, self.device_id))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub connector: String,
    /// The first mode the kernel lists, which is the preferred one. The mode
    /// actually in use cannot be read from sysfs.
    pub preferred_mode: String,
}

impl Output {
    pub fn display(&self) -> String {
        format!("{} {} (preferred)", self.connector, self.preferred_mode)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Battery {
    pub name: String,
    pub capacity: Option<u8>,
    pub status: Option<String>,
}

impl Battery {
    pub fn display(&self) -> String {
        match (self.capacity, &self.status) {
            (Some(capacity), Some(status)) => format!("{capacity}% ({status})"),
            (Some(capacity), None) => format!("{capacity}%"),
            (None, Some(status)) => status.clone(),
            (None, None) => "unknown".into(),
        }
    }
}

fn parse_hex_id(text: &str) -> Option<u16> {
    u16::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok()
}

/// Entries of `/sys/class/drm`, sorted so that card2 comes before card10.
fn drm_entries(sys: &Sys) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(sys.path("/sys/class/drm"))
        .map(|dir| {
            dir.filter_map(|entry| entry.ok())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| name.starts_with("card"))
                .collect()
        })
        .unwrap_or_default();
    names.sort_by_key(|name| {
        let digits: String = name[4..].chars().take_while(char::is_ascii_digit).collect();
        (digits.parse::<u32>().unwrap_or(u32::MAX), name.clone())
    });
    names
}

fn is_card(name: &str) -> bool {
    name.len() > 4 && name[4..].bytes().all(|b| b.is_ascii_digit())
}

pub fn gpus(sys: &Sys) -> Vec<Gpu> {
    let mut gpus: Vec<Gpu> = Vec::new();
    for card in drm_entries(sys).into_iter().filter(|name| is_card(name)) {
        let device = format!("/sys/class/drm/{card}/device");
        let (Some(vendor_id), Some(device_id)) = (
            parse_hex_id(&sys.read(&format!("{device}/vendor"))),
            parse_hex_id(&sys.read(&format!("{device}/device"))),
        ) else {
            continue;
        };
        if gpus
            .iter()
            .any(|gpu| gpu.vendor_id == vendor_id && gpu.device_id == device_id)
        {
            continue;
        }
        gpus.push(Gpu {
            card,
            vendor_id,
            device_id,
            name: None,
        });
    }
    if !gpus.is_empty() {
        let wanted: Vec<(u16, u16)> = gpus.iter().map(|g| (g.vendor_id, g.device_id)).collect();
        let names = pci_names(sys, &wanted);
        for (gpu, name) in gpus.iter_mut().zip(names) {
            gpu.name = name;
        }
    }
    gpus
}

/// Resolve PCI ids from pci.ids. Each vendor's block is found with one
/// substring search for its `\nvvvv  ` line, and only that block's device
/// lines are parsed, so the rest of the file is never split into lines.
pub fn pci_names(sys: &Sys, wanted: &[(u16, u16)]) -> Vec<Option<String>> {
    let mut found: Vec<Option<String>> = vec![None; wanted.len()];
    let Some(text) = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"]
        .iter()
        .find_map(|path| fs::read(sys.path(path)).ok())
        .map(utf8)
    else {
        return found;
    };
    for (index, &(vendor_id, device_id)) in wanted.iter().enumerate() {
        let needle = format!("\n{vendor_id:04x}  ");
        let Some(start) = text.find(&needle) else {
            continue;
        };
        let mut lines = text[start + 1..].lines();
        let Some(vendor_name) = lines.next().and_then(|line| line.get(6..)) else {
            continue;
        };
        for line in lines {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let Some(rest) = line.strip_prefix('\t') else {
                break; // the next vendor
            };
            if rest.starts_with('\t') {
                continue; // a subsystem line
            }
            let Some((id, name)) = rest.split_once("  ") else {
                continue;
            };
            if parse_hex_id(id) == Some(device_id) {
                found[index] = Some(gpu_name(vendor_id, vendor_name.trim(), name.trim()));
                break;
            }
        }
    }
    found
}

/// "Intel UHD Graphics 630", "AMD Radeon RX 6600/6600 XT/6600M": a short
/// vendor and the marketing name in brackets when pci.ids has one.
fn gpu_name(vendor_id: u16, vendor: &str, device: &str) -> String {
    let vendor = match vendor_id {
        0x1002 | 0x1022 => "AMD".to_string(),
        0x8086 => "Intel".to_string(),
        0x10de => "NVIDIA".to_string(),
        _ => vendor
            .trim_end_matches(" Corporation")
            .trim_end_matches(", Inc.")
            .trim_end_matches(" Inc.")
            .to_string(),
    };
    let device = match (device.rfind('['), device.rfind(']')) {
        (Some(open), Some(close)) if open < close => &device[open + 1..close],
        _ => device,
    };
    if device.starts_with(&vendor) {
        device.to_string()
    } else {
        format!("{vendor} {device}")
    }
}

/// Connected outputs and the first (preferred) mode of each.
pub fn outputs(sys: &Sys) -> Vec<Output> {
    drm_entries(sys)
        .into_iter()
        .filter(|name| !is_card(name) && name.contains('-'))
        .filter_map(|name| {
            let dir = format!("/sys/class/drm/{name}");
            if sys.read(&format!("{dir}/status")).trim() != "connected" {
                return None;
            }
            let modes = sys.read(&format!("{dir}/modes"));
            let mode = modes.lines().next()?.trim().to_string();
            if mode.is_empty() {
                return None;
            }
            let connector = name.split_once('-').map(|(_, c)| c).unwrap_or(&name);
            Some(Output {
                connector: connector.to_string(),
                preferred_mode: mode,
            })
        })
        .collect()
}

/// System batteries (`BAT*`, or any power supply of type Battery that is not
/// a peripheral's).
pub fn batteries(sys: &Sys) -> Vec<Battery> {
    let base = "/sys/class/power_supply";
    let mut names: Vec<String> = fs::read_dir(sys.path(base))
        .map(|dir| {
            dir.filter_map(|entry| entry.ok())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
        .into_iter()
        .filter(|name| {
            let kind = sys.read(&format!("{base}/{name}/type"));
            let scope = sys.read(&format!("{base}/{name}/scope"));
            (name.starts_with("BAT") || kind.trim() == "Battery") && scope.trim() != "Device"
        })
        .filter_map(|name| {
            let capacity = sys
                .read(&format!("{base}/{name}/capacity"))
                .trim()
                .parse::<u8>()
                .ok();
            let status = Some(
                sys.read(&format!("{base}/{name}/status"))
                    .trim()
                    .to_string(),
            )
            .filter(|status| !status.is_empty());
            (capacity.is_some() || status.is_some()).then_some(Battery {
                name,
                capacity,
                status,
            })
        })
        .collect()
}

/// Bytes as text without copying when they are valid UTF-8 (the fast,
/// normal case), so line splitting can use the optimised `str::lines`.
fn utf8(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

/// Count lines (trimmed) that match, given the previous line too.
fn count_lines(path: &Path, matches: impl Fn(&str, &str) -> bool) -> Option<usize> {
    let text = utf8(fs::read(path).ok()?);
    let mut previous = "";
    let mut count = 0;
    for line in text.lines() {
        let line = line.trim();
        if matches(previous, line) {
            count += 1;
        }
        previous = line;
    }
    Some(count)
}

/// Installed package counts read straight from the package databases:
/// dpkg `status`, the pacman `local/` directory and the xbps `pkgdb`.
pub fn packages(sys: &Sys) -> Vec<(&'static str, usize)> {
    let mut out = Vec::new();
    if let Ok(bytes) = fs::read(sys.path("/var/lib/dpkg/status")) {
        // Find the Status fields by substring search instead of splitting
        // every line of a multi-megabyte file.
        let text = utf8(bytes);
        let installed = |line: &str| {
            let line = line.lines().next().unwrap_or_default();
            line.trim_end().ends_with(" installed")
        };
        let first = text
            .strip_prefix("Status: ")
            .map(|rest| installed(rest) as usize)
            .unwrap_or(0);
        let rest = text
            .match_indices("\nStatus: ")
            .filter(|(at, needle)| installed(&text[at + needle.len()..]))
            .count();
        out.push(("dpkg", first + rest));
    }
    if let Ok(dir) = fs::read_dir(sys.path("/var/lib/pacman/local")) {
        let count = dir
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .count();
        out.push(("pacman", count));
    }
    if let Ok(dir) = fs::read_dir(sys.path("/var/db/xbps")) {
        let mut pkgdb: Vec<_> = dir
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("pkgdb-") && n.ends_with(".plist"))
                    .unwrap_or(false)
            })
            .collect();
        pkgdb.sort();
        if let Some(path) = pkgdb.last() {
            if let Some(count) = count_lines(path, |previous, line| {
                previous == "<key>state</key>" && line == "<string>installed</string>"
            }) {
                out.push(("xbps", count));
            }
        }
    }
    out
}

pub fn packages_display(packages: &[(&str, usize)]) -> String {
    packages
        .iter()
        .map(|(manager, count)| format!("{count} ({manager})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Swap used and total in bytes from /proc/meminfo.
pub fn swap(sys: &Sys) -> Option<(u64, u64)> {
    let text = sys.read("/proc/meminfo");
    let value = |key: &str| {
        text.lines()
            .find(|line| line.starts_with(key))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
    };
    let total = value("SwapTotal:")?;
    let free = value("SwapFree:")?;
    Some((total.saturating_sub(free) * 1024, total * 1024))
}

/// The 1, 5 and 15 minute load averages.
pub fn load(sys: &Sys) -> Option<[f64; 3]> {
    let text = sys.read("/proc/loadavg");
    let mut values = text.split_whitespace().map(|v| v.parse::<f64>().ok());
    Some([values.next()??, values.next()??, values.next()??])
}

/// Selected NIRUX extras with the version the installed generation pins for
/// each (`extras-selected` plus the `extras_table` in the generation's
/// `nirux` script, which `nirux extras status` prints as PINNED). Nothing is
/// executed; asking each extra for `--version` would cost one process each.
pub fn extras(state: &Path) -> Vec<(String, Option<String>)> {
    let selected = read(state.join("extras-selected"));
    let selected: Vec<&str> = selected
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    if selected.is_empty() {
        return Vec::new();
    }
    let script = read(state.join("current/bin/nirux"));
    let table = extras_table(&script);
    selected
        .into_iter()
        .map(|name| {
            let pinned = table
                .iter()
                .find(|(extra, _)| extra == name)
                .map(|(_, version)| version.clone());
            (name.to_string(), pinned)
        })
        .collect()
}

/// The `name repo version sha256` rows of `extras_table()` in the nirux script.
pub fn extras_table(script: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut lines = script.lines();
    if !lines.any(|line| line.trim_start().starts_with("extras_table()")) {
        return rows;
    }
    let mut inside = false;
    for line in lines {
        let line = line.trim();
        if !inside {
            if line.starts_with("cat <<") {
                inside = true;
                continue;
            }
            if line == "}" {
                break;
            }
            continue;
        }
        if line == "EOF" {
            break;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.len() == 4 {
            rows.push((words[0].to_string(), words[2].to_string()));
        }
    }
    rows
}

pub fn extras_display(extras: &[(String, Option<String>)]) -> String {
    extras
        .iter()
        .map(|(name, pinned)| match pinned {
            Some(version) => format!("{name} {version}"),
            None => name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Fixture;

    const PCI_IDS: &str = "\
# pci.ids sample
1002  Advanced Micro Devices, Inc. [AMD/ATI]
\t73ff  Navi 23 [Radeon RX 6600/6600 XT/6600M]
\t\t1002 0e3b  RX 6600 subsystem
10de  NVIDIA Corporation
\t2684  AD102 [GeForce RTX 4090]
1af4  Red Hat, Inc.
\t1050  Virtio 1.0 GPU
8086  Intel Corporation
\t9a49  TigerLake-LP GT2 [Iris Xe Graphics]
\t9be6  CometLake-S GT2 [UHD Graphics 630]
C 00  Unclassified device
";

    fn card(fixture: &Fixture, card: &str, vendor: &str, device: &str) {
        fixture.write(
            &format!("/sys/class/drm/{card}/device/vendor"),
            &format!("{vendor}\n"),
        );
        fixture.write(
            &format!("/sys/class/drm/{card}/device/device"),
            &format!("{device}\n"),
        );
    }

    fn connector(fixture: &Fixture, name: &str, status: &str, modes: &str) {
        fixture.write(&format!("/sys/class/drm/{name}/status"), status);
        fixture.write(&format!("/sys/class/drm/{name}/modes"), modes);
    }

    #[test]
    fn gpus_resolve_names_from_pci_ids() {
        let fixture = Fixture::new("gpu");
        card(&fixture, "card0", "0x8086", "0x9be6");
        card(&fixture, "card1", "0x1002", "0x73ff");
        card(&fixture, "card10", "0x1234", "0x5678");
        card(&fixture, "card2", "0x8086", "0x9be6"); // a duplicate
        fixture.write("/usr/share/misc/pci.ids", PCI_IDS);
        let shown = gpus_of(&fixture);
        assert_eq!(
            shown,
            [
                "Intel UHD Graphics 630",
                "AMD Radeon RX 6600/6600 XT/6600M",
                "1234:5678"
            ]
        );
        // hwdata's path is preferred and other vendors are shortened.
        let other = Fixture::new("gpu-hwdata");
        card(&other, "card0", "0x1af4", "0x1050");
        card(&other, "card1", "0x10de", "0x2684");
        other.write("/usr/share/hwdata/pci.ids", PCI_IDS);
        let shown: Vec<String> = gpus_of(&other);
        assert_eq!(shown, ["Red Hat Virtio 1.0 GPU", "NVIDIA GeForce RTX 4090"]);
        // Without pci.ids, the ids are shown.
        let bare = Fixture::new("gpu-bare");
        card(&bare, "card0", "0x8086", "0x9a49");
        assert_eq!(gpus_of(&bare), ["8086:9a49"]);
        assert!(gpus(&Fixture::new("gpu-none").sys()).is_empty());
    }

    fn gpus_of(fixture: &Fixture) -> Vec<String> {
        gpus(&fixture.sys()).iter().map(Gpu::display).collect()
    }

    #[test]
    fn outputs_list_connected_preferred_modes() {
        let fixture = Fixture::new("outputs");
        card(&fixture, "card1", "0x1002", "0x73ff");
        connector(
            &fixture,
            "card1-DP-5",
            "connected\n",
            "2560x1440\n1920x1080\n",
        );
        connector(&fixture, "card1-DP-6", "connected\n", "2560x1440\n");
        connector(&fixture, "card1-HDMI-A-4", "disconnected\n", "");
        connector(&fixture, "card1-Writeback-1", "unknown\n", "");
        connector(&fixture, "card0-eDP-1", "connected\n", ""); // no modes yet
        let shown: Vec<String> = outputs(&fixture.sys())
            .iter()
            .map(Output::display)
            .collect();
        assert_eq!(
            shown,
            ["DP-5 2560x1440 (preferred)", "DP-6 2560x1440 (preferred)"]
        );
    }

    #[test]
    fn batteries_read_capacity_and_status() {
        let fixture = Fixture::new("battery");
        let base = "/sys/class/power_supply";
        fixture.write(&format!("{base}/AC/type"), "Mains\n");
        fixture.write(&format!("{base}/AC/online"), "1\n");
        fixture.write(&format!("{base}/BAT0/type"), "Battery\n");
        fixture.write(&format!("{base}/BAT0/capacity"), "99\n");
        fixture.write(&format!("{base}/BAT0/status"), "Not charging\n");
        fixture.write(&format!("{base}/hidpp_battery_0/type"), "Battery\n");
        fixture.write(&format!("{base}/hidpp_battery_0/scope"), "Device\n");
        fixture.write(&format!("{base}/hidpp_battery_0/capacity"), "50\n");
        let shown: Vec<String> = batteries(&fixture.sys())
            .iter()
            .map(Battery::display)
            .collect();
        assert_eq!(shown, ["99% (Not charging)"]);
        assert!(batteries(&Fixture::new("battery-none").sys()).is_empty());
    }

    #[test]
    fn packages_are_counted_from_database_files() {
        let fixture = Fixture::new("packages");
        fixture.write(
            "/var/lib/dpkg/status",
            "Package: a\nStatus: install ok installed\n\nPackage: b\nStatus: deinstall ok config-files\n\n\
             Package: c\nStatus: hold ok installed\n\nPackage: d\nStatus: install ok not-installed\n",
        );
        fixture.write("/var/lib/pacman/local/ALPM_DB_VERSION", "9\n");
        fixture.write("/var/lib/pacman/local/7zip-26.05-1/desc", "x");
        fixture.write("/var/lib/pacman/local/bash-5.3-1/desc", "x");
        fixture.write(
            "/var/db/xbps/pkgdb-0.38.plist",
            "<dict>\n\t<key>7zip</key>\n\t<dict>\n\t\t<key>state</key>\n\t\t<string>installed</string>\n\t</dict>\n\
             \t<key>b</key>\n\t<dict>\n\t\t<key>state</key>\n\t\t<string>unpacked</string>\n\t</dict>\n\
             \t<key>c</key>\n\t<dict>\n\t\t<key>pkgver</key>\n\t\t<string>c-1</string>\n\t\t<key>state</key>\n\t\t<string>installed</string>\n\t</dict>\n</dict>\n",
        );
        let found = packages(&fixture.sys());
        assert_eq!(found, [("dpkg", 2), ("pacman", 2), ("xbps", 2)]);
        assert_eq!(packages_display(&found), "2 (dpkg), 2 (pacman), 2 (xbps)");
        assert!(packages(&Fixture::new("packages-none").sys()).is_empty());
        let first = Fixture::new("packages-first");
        first.write(
            "/var/lib/dpkg/status",
            "Status: install ok installed\nPackage: a\n\nPackage: b\nStatus: install ok installed\r\n",
        );
        assert_eq!(packages(&first.sys()), [("dpkg", 2)]);
    }

    #[test]
    fn swap_and_load_come_from_proc() {
        let fixture = Fixture::new("swap-load");
        fixture.write(
            "/proc/meminfo",
            "MemTotal: 100 kB\nSwapTotal:     2097148 kB\nSwapFree:      1048574 kB\n",
        );
        fixture.write("/proc/loadavg", "0.52 0.48 0.41 1/123 4567\n");
        assert_eq!(swap(&fixture.sys()), Some((1048574 * 1024, 2097148 * 1024)));
        assert_eq!(load(&fixture.sys()), Some([0.52, 0.48, 0.41]));
        let empty = Fixture::new("swap-load-none");
        assert_eq!(swap(&empty.sys()), None);
        assert_eq!(load(&empty.sys()), None);
    }

    #[test]
    fn extras_use_the_generation_pins() {
        let fixture = Fixture::new("extras");
        fixture.write("extras-selected", "niruamp\nnplay\nunknown\n");
        fixture.write(
            "current/bin/nirux",
            "#!/bin/sh\nVERSION=1\nextras_table() {\n  cat <<'EOF'\n\
             nplay nplay 1.8.0 aa10\nniruamp niruamp 0.3.3 3711\nEOF\n}\nother() {\n  cat <<'EOF'\nx y 9.9.9 z\nEOF\n}\n",
        );
        let found = extras(&fixture.root);
        assert_eq!(
            found,
            [
                ("niruamp".to_string(), Some("0.3.3".to_string())),
                ("nplay".to_string(), Some("1.8.0".to_string())),
                ("unknown".to_string(), None),
            ]
        );
        assert_eq!(
            extras_display(&found),
            "niruamp 0.3.3, nplay 1.8.0, unknown"
        );
        assert!(extras(&Fixture::new("extras-none").root).is_empty());
    }
}
