//! Botones de la consola y su traducción desde teclado (simulador) y gamepad (consola).

use std::collections::HashMap;
use std::path::Path;

use sdl2::controller::{Axis, Button as PadButton};
use sdl2::keyboard::Keycode;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    Select,
    Start,
    Fn,
}

impl Button {
    pub const ALL: [Button; 15] = [
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
        Button::L1,
        Button::R1,
        Button::L2,
        Button::R2,
        Button::Select,
        Button::Start,
        Button::Fn,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&b| b == self).unwrap_or(0)
    }

    pub fn from_config_name(name: &str) -> Option<Button> {
        Some(match name {
            "up" => Button::Up,
            "down" => Button::Down,
            "left" => Button::Left,
            "right" => Button::Right,
            "a" => Button::A,
            "b" => Button::B,
            "x" => Button::X,
            "y" => Button::Y,
            "l1" => Button::L1,
            "r1" => Button::R1,
            "l2" => Button::L2,
            "r2" => Button::R2,
            "select" => Button::Select,
            "start" => Button::Start,
            "fn" => Button::Fn,
            _ => return None,
        })
    }

    /// Botones del GameController de SDL. SDL usa nombres posicionales de Xbox (A abajo);
    /// en la RGB20S, qué botón físico llega como cada uno depende del mapeo de dArkOS
    /// (sin verificar). Los gatillos analógicos llegan como ejes, no como botones.
    pub fn from_pad(b: PadButton) -> Option<Button> {
        Some(match b {
            PadButton::DPadUp => Button::Up,
            PadButton::DPadDown => Button::Down,
            PadButton::DPadLeft => Button::Left,
            PadButton::DPadRight => Button::Right,
            PadButton::A => Button::A,
            PadButton::B => Button::B,
            PadButton::X => Button::X,
            PadButton::Y => Button::Y,
            PadButton::LeftShoulder => Button::L1,
            PadButton::RightShoulder => Button::R1,
            PadButton::Back => Button::Select,
            PadButton::Start => Button::Start,
            PadButton::Guide => Button::Fn,
            _ => return None,
        })
    }
}

/// Copia de `infra/sim/keymap.toml`, para cuando la app corre sin `RGB20S_KEYMAP`.
const DEFAULT_KEYS: [(Button, &str); 15] = [
    (Button::Up, "Up"),
    (Button::Down, "Down"),
    (Button::Left, "Left"),
    (Button::Right, "Right"),
    (Button::A, "X"),
    (Button::B, "Z"),
    (Button::X, "S"),
    (Button::Y, "A"),
    (Button::L1, "Q"),
    (Button::R1, "W"),
    (Button::L2, "1"),
    (Button::R2, "2"),
    (Button::Select, "Right Shift"),
    (Button::Start, "Return"),
    (Button::Fn, "Escape"),
];

/// Copia de las secciones `[left_stick]` y `[right_stick]` de `infra/sim/keymap.toml`.
const DEFAULT_STICK_KEYS: [(Stick, StickDir, &str); 8] = [
    (Stick::Left, StickDir::Up, "I"),
    (Stick::Left, StickDir::Down, "K"),
    (Stick::Left, StickDir::Left, "J"),
    (Stick::Left, StickDir::Right, "L"),
    (Stick::Right, StickDir::Up, "Keypad 8"),
    (Stick::Right, StickDir::Down, "Keypad 2"),
    (Stick::Right, StickDir::Left, "Keypad 4"),
    (Stick::Right, StickDir::Right, "Keypad 6"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stick {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StickDir {
    Up,
    Down,
    Left,
    Right,
}

impl StickDir {
    fn from_config_name(name: &str) -> Option<StickDir> {
        Some(match name {
            "up" => StickDir::Up,
            "down" => StickDir::Down,
            "left" => StickDir::Left,
            "right" => StickDir::Right,
            _ => return None,
        })
    }
}

#[derive(Deserialize)]
struct KeymapFile {
    buttons: HashMap<String, String>,
    #[serde(default)]
    left_stick: HashMap<String, String>,
    #[serde(default)]
    right_stick: HashMap<String, String>,
}

pub struct Keymap {
    keys: HashMap<Keycode, Button>,
    stick_keys: HashMap<Keycode, (Stick, StickDir)>,
}

impl Keymap {
    pub fn builtin() -> Self {
        let keys = DEFAULT_KEYS
            .iter()
            .filter_map(|&(button, name)| Some((Keycode::from_name(name)?, button)))
            .collect();
        let stick_keys = DEFAULT_STICK_KEYS
            .iter()
            .filter_map(|&(stick, dir, name)| Some((Keycode::from_name(name)?, (stick, dir))))
            .collect();
        Self { keys, stick_keys }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, String> {
        let file: KeymapFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut keys = HashMap::new();
        for (name, key) in &file.buttons {
            let button = Button::from_config_name(name)
                .ok_or_else(|| format!("botón desconocido en el keymap: {name}"))?;
            let code = Keycode::from_name(key)
                .ok_or_else(|| format!("tecla desconocida para {name}: {key}"))?;
            keys.insert(code, button);
        }
        let mut stick_keys = HashMap::new();
        for (stick, section) in [
            (Stick::Left, &file.left_stick),
            (Stick::Right, &file.right_stick),
        ] {
            for (name, key) in section {
                let dir = StickDir::from_config_name(name)
                    .ok_or_else(|| format!("dirección de stick desconocida: {name}"))?;
                let code = Keycode::from_name(key)
                    .ok_or_else(|| format!("tecla desconocida para el stick {name}: {key}"))?;
                stick_keys.insert(code, (stick, dir));
            }
        }
        Ok(Self { keys, stick_keys })
    }

    pub fn button(&self, key: Keycode) -> Option<Button> {
        self.keys.get(&key).copied()
    }

    pub fn stick(&self, key: Keycode) -> Option<(Stick, StickDir)> {
        self.stick_keys.get(&key).copied()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }
}

/// Estado de los dos sticks: teclas del simulador o ejes del gamepad. Arriba es positivo.
pub struct Sticks {
    dead_zone: f32,
    keys: [[bool; 4]; 2],
    axes: [[f32; 2]; 2],
}

impl Sticks {
    pub fn new(dead_zone: f32) -> Self {
        Self {
            dead_zone,
            keys: [[false; 4]; 2],
            axes: [[0.0; 2]; 2],
        }
    }

    pub fn set_key(&mut self, stick: Stick, dir: StickDir, down: bool) {
        self.keys[stick as usize][dir as usize] = down;
    }

    pub fn set_axis(&mut self, stick: Stick, vertical: bool, raw: i16) {
        let v = (f32::from(raw) / 32767.0).clamp(-1.0, 1.0);
        self.axes[stick as usize][usize::from(vertical)] = if vertical { -v } else { v };
    }

    /// Ejes de un gamepad: devuelve el stick y si el eje es vertical.
    pub fn axis_of(axis: Axis) -> Option<(Stick, bool)> {
        match axis {
            Axis::LeftX => Some((Stick::Left, false)),
            Axis::LeftY => Some((Stick::Left, true)),
            Axis::RightX => Some((Stick::Right, false)),
            Axis::RightY => Some((Stick::Right, true)),
            _ => None,
        }
    }

    /// `(x, y)` en -1..=1, con la zona muerta aplicada.
    pub fn value(&self, stick: Stick) -> (f32, f32) {
        let k = self.keys[stick as usize];
        let key_axis = |neg: bool, pos: bool| f32::from(u8::from(pos)) - f32::from(u8::from(neg));
        let kx = key_axis(k[StickDir::Left as usize], k[StickDir::Right as usize]);
        let ky = key_axis(k[StickDir::Down as usize], k[StickDir::Up as usize]);
        if kx != 0.0 || ky != 0.0 {
            return (kx, ky);
        }
        let [x, y] = self.axes[stick as usize];
        (self.shape(x), self.shape(y))
    }

    fn shape(&self, v: f32) -> f32 {
        let m = v.abs();
        if m <= self.dead_zone {
            0.0
        } else {
            v.signum() * (m - self.dead_zone) / (1.0 - self.dead_zone)
        }
    }
}

/// L2/R2 analógicos que llegan como ejes: se convierten en botones con histéresis.
#[derive(Default)]
pub struct Triggers {
    down: [bool; 2],
}

impl Triggers {
    const PRESS: i16 = 16_000;
    const RELEASE: i16 = 8_000;

    /// Devuelve el botón que cambió de estado, si cambió.
    pub fn update(&mut self, axis: Axis, value: i16) -> Option<(Button, bool)> {
        let (i, button) = match axis {
            Axis::TriggerLeft => (0, Button::L2),
            Axis::TriggerRight => (1, Button::R2),
            _ => return None,
        };
        let down = &mut self.down[i];
        if !*down && value >= Self::PRESS {
            *down = true;
            Some((button, true))
        } else if *down && value <= Self::RELEASE {
            *down = false;
            Some((button, false))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_keymap_parses() {
        let text = include_str!("../../../infra/sim/keymap.toml");
        let map = Keymap::parse(text).expect("keymap del repo válido");
        assert_eq!(map.len(), 15);
        assert_eq!(map.button(Keycode::X), Some(Button::A));
        assert_eq!(map.button(Keycode::RShift), Some(Button::Select));
        assert_eq!(map.button(Keycode::Return), Some(Button::Start));
    }

    #[test]
    fn builtin_matches_repo_keymap() {
        let repo = Keymap::parse(include_str!("../../../infra/sim/keymap.toml")).unwrap();
        let builtin = Keymap::builtin();
        assert_eq!(builtin.keys, repo.keys);
        assert_eq!(builtin.stick_keys, repo.stick_keys);
        assert_eq!(builtin.stick_keys.len(), 8);
    }

    #[test]
    fn stick_values_with_dead_zone() {
        let mut s = Sticks::new(0.15);
        assert_eq!(s.value(Stick::Left), (0.0, 0.0));
        s.set_key(Stick::Left, StickDir::Up, true);
        s.set_key(Stick::Left, StickDir::Right, true);
        assert_eq!(s.value(Stick::Left), (1.0, 1.0), "arriba es positivo");
        s.set_key(Stick::Left, StickDir::Up, false);
        s.set_key(Stick::Left, StickDir::Right, false);

        // Dentro de la zona muerta: cero. Fuera: reescalado para arrancar en 0.
        s.set_axis(Stick::Right, false, (0.1 * 32767.0) as i16);
        assert_eq!(s.value(Stick::Right).0, 0.0);
        s.set_axis(Stick::Right, false, i16::MAX);
        assert!((s.value(Stick::Right).0 - 1.0).abs() < 1e-3);
        // El eje Y del SDL crece hacia abajo.
        s.set_axis(Stick::Right, true, i16::MAX);
        assert!(s.value(Stick::Right).1 < -0.99);
    }

    #[test]
    fn trigger_hysteresis() {
        let mut t = Triggers::default();
        assert_eq!(t.update(Axis::TriggerLeft, 10_000), None);
        assert_eq!(
            t.update(Axis::TriggerLeft, 20_000),
            Some((Button::L2, true))
        );
        assert_eq!(t.update(Axis::TriggerLeft, 12_000), None);
        assert_eq!(
            t.update(Axis::TriggerLeft, 5_000),
            Some((Button::L2, false))
        );
        assert_eq!(t.update(Axis::LeftX, 30_000), None);
    }

    #[test]
    fn unknown_button_is_error() {
        assert!(Keymap::parse("[buttons]\nturbo = \"X\"\n").is_err());
    }
}
