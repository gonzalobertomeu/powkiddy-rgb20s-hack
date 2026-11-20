//! Carga de la batería desde `/sys/class/power_supply/*/` (la primera con `type` = `Battery`).

use std::path::Path;

pub const POWER_SUPPLY_DIR: &str = "/sys/class/power_supply";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}

impl Battery {
    pub fn label(&self) -> String {
        let sign = if self.charging { "+" } else { "" };
        format!("{sign}{}%", self.percent)
    }
}

fn read_trimmed(path: &Path) -> Option<String> {
    Some(std::fs::read_to_string(path).ok()?.trim().to_string())
}

pub fn read(base: &Path) -> Option<Battery> {
    let mut dirs: Vec<_> = std::fs::read_dir(base)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    dirs.sort();
    dirs.into_iter().find_map(|dir| {
        if read_trimmed(&dir.join("type"))? != "Battery" {
            return None;
        }
        let capacity: u32 = read_trimmed(&dir.join("capacity"))?.parse().ok()?;
        let status = read_trimmed(&dir.join("status")).unwrap_or_default();
        Some(Battery {
            percent: capacity.min(100) as u8,
            charging: status == "Charging",
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn supply(base: &Path, name: &str, files: &[(&str, &str)]) {
        let dir = base.join(name);
        fs::create_dir_all(&dir).unwrap();
        for (f, v) in files {
            fs::write(dir.join(f), format!("{v}\n")).unwrap();
        }
    }

    #[test]
    fn finds_battery_and_skips_mains() {
        let base = std::env::temp_dir().join(format!("launcher-bat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        assert_eq!(read(&base), None);
        supply(&base, "ac", &[("type", "Mains"), ("online", "1")]);
        assert_eq!(read(&base), None);
        supply(
            &base,
            "battery",
            &[
                ("type", "Battery"),
                ("capacity", "87"),
                ("status", "Charging"),
            ],
        );
        let b = read(&base).unwrap();
        assert_eq!(
            b,
            Battery {
                percent: 87,
                charging: true
            }
        );
        assert_eq!(b.label(), "+87%");
        supply(
            &base,
            "battery",
            &[("capacity", "130"), ("status", "Discharging")],
        );
        assert_eq!(read(&base).unwrap().label(), "100%");
        fs::remove_dir_all(&base).unwrap();
    }
}
