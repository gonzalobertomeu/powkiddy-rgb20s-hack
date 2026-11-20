//! Botones de la consola y su traducción desde teclado (simulador) y gamepad (consola).
//! Versión reducida de `projects/groovebox/src/input.rs`: sin sticks ni gatillos analógicos.

use std::collections::HashMap;
use std::path::Path;

use sdl2::controller::Button as PadButton;
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

    /// Botones del GameController de SDL (nombres posicionales de Xbox), igual que el groovebox.
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

/// Copia de la sección `[buttons]` de `infra/sim/keymap.toml`, para cuando falta `RGB20S_KEYMAP`.
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

/// Solo interesa `[buttons]`; las secciones de los sticks se ignoran.
#[derive(Deserialize)]
struct KeymapFile {
    buttons: HashMap<String, String>,
}

pub struct Keymap {
    keys: HashMap<Keycode, Button>,
}

impl Keymap {
    pub fn builtin() -> Self {
        let keys = DEFAULT_KEYS
            .iter()
            .filter_map(|&(button, name)| Some((Keycode::from_name(name)?, button)))
            .collect();
        Self { keys }
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
        Ok(Self { keys })
    }

    pub fn button(&self, key: Keycode) -> Option<Button> {
        self.keys.get(&key).copied()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_keymaps_parse() {
        let map = Keymap::parse(include_str!("../../../infra/sim/keymap.toml")).unwrap();
        assert_eq!(map.len(), 15);
        assert_eq!(map.button(Keycode::X), Some(Button::A));
        assert_eq!(map.button(Keycode::Up), Some(Button::Up));
        let vim = Keymap::parse(include_str!("../../../infra/sim/keymap-vim.toml")).unwrap();
        assert!(vim.len() >= 4);
    }

    #[test]
    fn builtin_matches_repo_keymap() {
        let repo = Keymap::parse(include_str!("../../../infra/sim/keymap.toml")).unwrap();
        assert_eq!(Keymap::builtin().keys, repo.keys);
    }
}
