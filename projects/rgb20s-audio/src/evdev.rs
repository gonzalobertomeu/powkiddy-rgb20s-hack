//! Lectura de los botones de volumen desde evdev, sin dependencias: se leen los
//! `struct input_event` crudos del dispositivo.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::gesture::{KeyAction, VolumeKey};

const EV_KEY: u16 = 1;
const KEY_VOLUMEDOWN: u16 = 114;
const KEY_VOLUMEUP: u16 = 115;

/// `struct input_event` con `struct timeval` de 64 bits (aarch64 y x86_64): 16 + 2 + 2 + 4.
#[cfg(target_pointer_width = "64")]
pub const EVENT_SIZE: usize = 24;
#[cfg(target_pointer_width = "32")]
pub const EVENT_SIZE: usize = 16;
const HEADER: usize = EVENT_SIZE - 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputEvent {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

impl InputEvent {
    pub fn parse(raw: &[u8]) -> Option<InputEvent> {
        let raw = raw.get(..EVENT_SIZE)?;
        Some(InputEvent {
            kind: u16::from_ne_bytes([raw[HEADER], raw[HEADER + 1]]),
            code: u16::from_ne_bytes([raw[HEADER + 2], raw[HEADER + 3]]),
            value: i32::from_ne_bytes(raw[HEADER + 4..HEADER + 8].try_into().ok()?),
        })
    }

    /// Botón de volumen y acción, si el evento es uno.
    pub fn volume_key(&self) -> Option<(VolumeKey, KeyAction)> {
        if self.kind != EV_KEY {
            return None;
        }
        let key = match self.code {
            KEY_VOLUMEUP => VolumeKey::Up,
            KEY_VOLUMEDOWN => VolumeKey::Down,
            _ => return None,
        };
        let action = match self.value {
            0 => KeyAction::Release,
            1 => KeyAction::Press,
            2 => KeyAction::Repeat,
            _ => return None,
        };
        Some((key, action))
    }
}

/// Busca `/dev/input/eventN` cuyo `name` en sysfs sea `name`.
pub fn find_device(sys_class_input: &Path, dev_input: &Path, name: &str) -> Option<PathBuf> {
    let mut found: Vec<String> = fs::read_dir(sys_class_input)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_name = entry.file_name().into_string().ok()?;
            if !file_name.starts_with("event") {
                return None;
            }
            let dev_name = fs::read_to_string(entry.path().join("device/name")).ok()?;
            (dev_name.trim_end() == name).then_some(file_name)
        })
        .collect();
    found.sort();
    found.first().map(|f| dev_input.join(f))
}

/// Dispositivo abierto, que entrega eventos de a uno (bloquea en `read`).
pub struct Device {
    file: File,
    buf: [u8; EVENT_SIZE * 16],
    len: usize,
    pos: usize,
}

impl Device {
    pub fn open(path: &Path) -> io::Result<Device> {
        Ok(Device {
            file: File::open(path)?,
            buf: [0; EVENT_SIZE * 16],
            len: 0,
            pos: 0,
        })
    }

    pub fn next_event(&mut self) -> io::Result<InputEvent> {
        loop {
            if self.pos + EVENT_SIZE <= self.len {
                let event = InputEvent::parse(&self.buf[self.pos..self.len]);
                self.pos += EVENT_SIZE;
                if let Some(event) = event {
                    return Ok(event);
                }
            }
            // evdev siempre entrega eventos enteros.
            self.len = self.file.read(&mut self.buf)?;
            self.pos = 0;
            if self.len == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "fin del dispositivo",
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_event(kind: u16, code: u16, value: i32) -> Vec<u8> {
        let mut raw = vec![0xAA; HEADER];
        raw.extend(kind.to_ne_bytes());
        raw.extend(code.to_ne_bytes());
        raw.extend(value.to_ne_bytes());
        raw
    }

    #[test]
    fn parses_volume_keys() {
        let up = InputEvent::parse(&raw_event(EV_KEY, 115, 1)).unwrap();
        assert_eq!(
            up,
            InputEvent {
                kind: 1,
                code: 115,
                value: 1
            }
        );
        assert_eq!(up.volume_key(), Some((VolumeKey::Up, KeyAction::Press)));
        let down = InputEvent::parse(&raw_event(EV_KEY, 114, 2)).unwrap();
        assert_eq!(
            down.volume_key(),
            Some((VolumeKey::Down, KeyAction::Repeat))
        );
        let release = InputEvent::parse(&raw_event(EV_KEY, 114, 0)).unwrap();
        assert_eq!(
            release.volume_key(),
            Some((VolumeKey::Down, KeyAction::Release))
        );
    }

    #[test]
    fn ignores_other_events() {
        // EV_SYN, otra tecla (power), EV_SW del jack
        for (kind, code, value) in [(0, 0, 0), (1, 116, 1), (5, 2, 1)] {
            let event = InputEvent::parse(&raw_event(kind, code, value)).unwrap();
            assert_eq!(event.volume_key(), None);
        }
        assert_eq!(InputEvent::parse(&[0; EVENT_SIZE - 1]), None);
    }

    #[test]
    fn reads_events_from_a_stream() {
        let dir = std::env::temp_dir().join(format!("rgb20s-audio-ev-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("event2");
        let mut data = raw_event(EV_KEY, 115, 1);
        data.extend(raw_event(0, 0, 0));
        data.extend(raw_event(EV_KEY, 115, 0));
        fs::write(&path, data).unwrap();

        let mut dev = Device::open(&path).unwrap();
        assert_eq!(dev.next_event().unwrap().value, 1);
        assert_eq!(dev.next_event().unwrap().kind, 0);
        assert_eq!(dev.next_event().unwrap().value, 0);
        assert!(dev.next_event().is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_device_by_name() {
        let root = std::env::temp_dir().join(format!("rgb20s-audio-sys-{}", std::process::id()));
        for (event, name) in [
            ("event0", "rk805 pwrkey"),
            ("event2", "gpio-keys-vol"),
            ("event3", "r36s_Gamepad"),
        ] {
            let dir = root.join(event).join("device");
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("name"), format!("{name}\n")).unwrap();
        }
        fs::create_dir_all(root.join("input4")).unwrap();

        let dev = Path::new("/dev/input");
        assert_eq!(
            find_device(&root, dev, "gpio-keys-vol"),
            Some(PathBuf::from("/dev/input/event2"))
        );
        assert_eq!(find_device(&root, dev, "gpio-keys"), None);
        fs::remove_dir_all(&root).unwrap();
    }
}
