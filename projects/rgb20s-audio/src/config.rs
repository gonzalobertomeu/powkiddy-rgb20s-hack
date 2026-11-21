use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io};

use serde::Deserialize;

use crate::mode::Mode;

pub const DEFAULT_PATH: &str = "/etc/rgb20s-audio.toml";

/// Configuración del servicio (`/etc/rgb20s-audio.toml`). Todos los campos son opcionales.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Tarjeta ALSA: índice o id (`hw:<card>`).
    pub card: String,
    /// Nombre del dispositivo evdev de los botones (`/sys/class/input/event*/device/name`).
    pub input_device: String,
    /// Elemento simple del mixer; solo se escribe su parte de reproducción.
    pub volume_control: String,
    pub volume_step_db: f64,
    /// Elemento enumerado que se fuerza al arrancar ("" = no tocar), y el ítem que se elige.
    pub mux_control: String,
    pub mux_item: String,
    /// Ventana para el gesto de los dos botones a la vez.
    pub gesture_window_ms: u64,
    /// Espera desde el último cambio de volumen hasta `alsactl store`.
    pub store_delay_ms: u64,
    pub alsactl: PathBuf,
    /// Modo si no hay estado guardado.
    pub default_mode: Mode,
    pub state_dir: PathBuf,
    pub alsa_conf_path: PathBuf,
    /// Destino de la cadena de `default`; por defecto `plughw:<card>,0`.
    pub pcm_slave: Option<String>,
    pub jack: JackConfig,
}

/// Detección de auriculares por el control ALSA del jack (hoy no funciona en la RGB20S).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JackConfig {
    pub enabled: bool,
    /// Control booleano de interfaz CARD.
    pub control: String,
    /// Valor del control con los auriculares enchufados (polaridad).
    pub inserted_value: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            card: "0".into(),
            input_device: "gpio-keys-vol".into(),
            volume_control: "Master".into(),
            volume_step_db: 3.0,
            mux_control: "Playback Mux".into(),
            mux_item: "HP".into(),
            gesture_window_ms: 150,
            store_delay_ms: 2000,
            alsactl: "/usr/bin/alsactl".into(),
            default_mode: Mode::Mono,
            state_dir: "/var/lib/rgb20s-audio".into(),
            alsa_conf_path: "/var/lib/alsa/conf.d/60-rgb20s-audio.conf".into(),
            pcm_slave: None,
            jack: JackConfig::default(),
        }
    }
}

impl Default for JackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            control: "Headphones Jack".into(),
            inserted_value: true,
        }
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let config: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    /// Lee el archivo; si no existe, usa los valores por defecto. El `bool` dice si existía.
    pub fn load(path: &Path) -> Result<(Config, bool), String> {
        match fs::read_to_string(path) {
            Ok(text) => Config::parse(&text)
                .map(|c| (c, true))
                .map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok((Config::default(), false)),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if !(self.volume_step_db > 0.0 && self.volume_step_db <= 20.0) {
            return Err(format!(
                "volume_step_db fuera de rango (0..20]: {}",
                self.volume_step_db
            ));
        }
        if !(20..=1000).contains(&self.gesture_window_ms) {
            return Err(format!(
                "gesture_window_ms fuera de rango (20..1000): {}",
                self.gesture_window_ms
            ));
        }
        // Van entre comillas dentro de la configuración de ALSA.
        for (name, value) in [
            ("card", Some(&self.card)),
            ("pcm_slave", self.pcm_slave.as_ref()),
        ] {
            if let Some(v) = value
                && (v.is_empty() || v.contains(['"', '\\', '\n']))
            {
                return Err(format!("{name} inválido: {v:?}"));
            }
        }
        Ok(())
    }

    pub fn pcm_slave(&self) -> String {
        self.pcm_slave
            .clone()
            .unwrap_or_else(|| format!("plughw:{},0", self.card))
    }

    pub fn ctl_name(&self) -> String {
        format!("hw:{}", self.card)
    }

    pub fn gesture_window(&self) -> Duration {
        Duration::from_millis(self.gesture_window_ms)
    }

    pub fn store_delay(&self) -> Duration {
        Duration::from_millis(self.store_delay_ms)
    }

    pub fn mode_file(&self) -> PathBuf {
        self.state_dir.join("mode")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        let c = Config::parse("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.pcm_slave(), "plughw:0,0");
        assert_eq!(c.ctl_name(), "hw:0");
        assert_eq!(c.default_mode, Mode::Mono);
        assert!(!c.jack.enabled);
        assert_eq!(c.mode_file(), PathBuf::from("/var/lib/rgb20s-audio/mode"));
    }

    #[test]
    fn partial_file_overrides_fields() {
        let c = Config::parse(
            r#"
            card = "rk817int"
            volume_step_db = 2.5
            default_mode = "stereo"

            [jack]
            enabled = true
            inserted_value = false
            "#,
        )
        .unwrap();
        assert_eq!(c.pcm_slave(), "plughw:rk817int,0");
        assert_eq!(c.volume_step_db, 2.5);
        assert_eq!(c.default_mode, Mode::Stereo);
        assert!(c.jack.enabled);
        assert!(!c.jack.inserted_value);
        assert_eq!(c.jack.control, "Headphones Jack");
        assert_eq!(c.gesture_window(), Duration::from_millis(150));
    }

    #[test]
    fn rejects_unknown_fields_and_bad_values() {
        assert!(Config::parse("volumen = 3").is_err());
        assert!(Config::parse("default_mode = \"surround\"").is_err());
        assert!(Config::parse("volume_step_db = 0").is_err());
        assert!(Config::parse("gesture_window_ms = 5").is_err());
        assert!(Config::parse("pcm_slave = 'hw:0\" x'").is_err());
        assert!(Config::parse("card = ''").is_err());
    }

    #[test]
    fn example_file_matches_defaults() {
        let c = Config::parse(include_str!("../system/rgb20s-audio.toml")).unwrap();
        assert_eq!(c, Config::default());
        let uncommented: String = include_str!("../system/rgb20s-audio.toml")
            .lines()
            .map(|l| {
                l.strip_prefix("# ")
                    .filter(|l| {
                        l.split_whitespace().nth(1) == Some("=")
                            && l.starts_with(|c: char| c.is_ascii_lowercase())
                    })
                    .unwrap_or(l)
            })
            .map(|l| format!("{}\n", l.split("  #").next().unwrap()))
            .collect();
        let mut explicit = Config::parse(&uncommented).unwrap();
        assert_eq!(explicit.pcm_slave(), Config::default().pcm_slave());
        explicit.pcm_slave = None;
        assert_eq!(explicit, Config::default());
    }

    #[test]
    fn missing_file_is_not_an_error() {
        let (c, found) = Config::load(Path::new("/nonexistent/rgb20s-audio.toml")).unwrap();
        assert!(!found);
        assert_eq!(c, Config::default());
    }
}
