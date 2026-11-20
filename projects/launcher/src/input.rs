//! Botones de la consola y su traducción desde teclado (simulador) y gamepad (consola).
//! Versión reducida de `projects/groovebox/src/input.rs`: sin sticks ni gatillos analógicos.

use std::collections::HashMap;
use std::path::Path;

use sdl2::controller::Button as PadButton;
use sdl2::keyboard::Keycode;
use serde::Deserialize;

/// Disposición de los botones frontales. La RGB20S usa la de Nintendo: A a la derecha, B abajo,
/// Y a la izquierda y X arriba. SDL nombra los botones del GameController por posición, al estilo
/// Xbox (A abajo, B derecha, X izquierda, Y arriba), así que con `Nintendo` se intercambian A↔B y
/// X↔Y. Si en la consola los botones llegan con las etiquetas cambiadas, `RGB20S_PAD_LAYOUT=xbox`
/// usa los nombres de SDL tal cual (sin verificar en el hardware).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadLayout {
    Nintendo,
    Xbox,
}

impl PadLayout {
    /// Lee `RGB20S_PAD_LAYOUT` (`nintendo` o `xbox`) una sola vez; por defecto, `Nintendo`.
    pub fn current() -> PadLayout {
        static LAYOUT: std::sync::OnceLock<PadLayout> = std::sync::OnceLock::new();
        *LAYOUT.get_or_init(|| Self::parse(std::env::var("RGB20S_PAD_LAYOUT").ok().as_deref()))
    }

    fn parse(value: Option<&str>) -> PadLayout {
        match value.map(str::to_ascii_lowercase).as_deref() {
            Some("xbox") => PadLayout::Xbox,
            _ => PadLayout::Nintendo,
        }
    }
}

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

    /// Botón del GameController de SDL según la disposición de la consola (`PadLayout::current`).
    pub fn from_pad(b: PadButton) -> Option<Button> {
        Self::from_pad_with(b, PadLayout::current())
    }

    /// SDL usa nombres posicionales de Xbox; ver `PadLayout`. Los gatillos analógicos llegan como
    /// ejes, no como botones.
    pub fn from_pad_with(b: PadButton, layout: PadLayout) -> Option<Button> {
        let nintendo = layout == PadLayout::Nintendo;
        Some(match b {
            PadButton::DPadUp => Button::Up,
            PadButton::DPadDown => Button::Down,
            PadButton::DPadLeft => Button::Left,
            PadButton::DPadRight => Button::Right,
            PadButton::A if nintendo => Button::B,
            PadButton::A => Button::A,
            PadButton::B if nintendo => Button::A,
            PadButton::B => Button::B,
            PadButton::X if nintendo => Button::Y,
            PadButton::X => Button::X,
            PadButton::Y if nintendo => Button::X,
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
    fn pad_layout_nintendo_swaps_face_buttons() {
        let n = PadLayout::Nintendo;
        assert_eq!(Button::from_pad_with(PadButton::B, n), Some(Button::A)); // derecha
        assert_eq!(Button::from_pad_with(PadButton::A, n), Some(Button::B)); // abajo
        assert_eq!(Button::from_pad_with(PadButton::X, n), Some(Button::Y)); // izquierda
        assert_eq!(Button::from_pad_with(PadButton::Y, n), Some(Button::X)); // arriba
        assert_eq!(
            Button::from_pad_with(PadButton::DPadUp, n),
            Some(Button::Up)
        );
    }

    #[test]
    fn pad_layout_xbox_keeps_sdl_names() {
        let x = PadLayout::Xbox;
        assert_eq!(Button::from_pad_with(PadButton::A, x), Some(Button::A));
        assert_eq!(Button::from_pad_with(PadButton::Y, x), Some(Button::Y));
    }

    #[test]
    fn pad_layout_parse_defaults_to_nintendo() {
        assert_eq!(PadLayout::parse(None), PadLayout::Nintendo);
        assert_eq!(PadLayout::parse(Some("XBOX")), PadLayout::Xbox);
        assert_eq!(PadLayout::parse(Some("otra")), PadLayout::Nintendo);
    }

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
