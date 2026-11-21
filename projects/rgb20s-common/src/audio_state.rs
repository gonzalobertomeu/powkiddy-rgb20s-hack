//! Estado del audio que publica el servicio `rgb20s-audio` en `/run/rgb20s-audio/state`.
//!
//! Formato: líneas `clave=valor` en UTF-8. El lector ignora las claves que no conoce, las
//! líneas vacías y las que empiezan con `#`, así se pueden agregar campos sin romper apps viejas.
//!
//! ```text
//! volume_pct=66
//! volume_db=-20.1
//! mode=mono
//! seq=3
//! ```
//!
//! - `volume_pct` (obligatorio): 0..=100, lineal en dB (ver [`volume_percent`]).
//! - `volume_db`: el valor real del control, informativo.
//! - `mode` (obligatorio): `mono` | `stereo`.
//! - `seq`: sube con cada pulsación atendida (también en los extremos, donde el volumen no
//!   cambia) y con cada cambio de modo; vuelve a empezar si se reinicia el servicio.
//!
//! El servicio lo escribe con un renombrado atómico: un lector ve el archivo viejo o el nuevo.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::Deserialize;

pub const STATE_PATH: &str = "/run/rgb20s-audio/state";
/// Variable de entorno que reemplaza [`STATE_PATH`] (simulador, pruebas).
pub const STATE_ENV: &str = "RGB20S_AUDIO_STATE";

/// Rango que cubre la escala de 0 a 100 %, por debajo del máximo del control. Con pasos de
/// 3 dB, cada pulsación mueve un 5 %; por debajo de -60 dB ya casi no se oye y se muestra 0 %.
pub const DISPLAY_RANGE_DB: f64 = 60.0;

/// Ruta del estado: `$RGB20S_AUDIO_STATE` si está definida y no vacía, si no [`STATE_PATH`].
pub fn state_path() -> PathBuf {
    std::env::var_os(STATE_ENV)
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from(STATE_PATH), PathBuf::from)
}

/// Porcentaje para mostrar: lineal en dB entre `db_max - DISPLAY_RANGE_DB` (0 %) y `db_max`
/// (100 %). El oído percibe el volumen en dB, así que pasos iguales dan saltos iguales.
pub fn volume_percent(db: f64, db_max: f64) -> u8 {
    let t = (db - (db_max - DISPLAY_RANGE_DB)) / DISPLAY_RANGE_DB;
    (t * 100.0).round().clamp(0.0, 100.0) as u8
}

/// Cómo se mezclan los canales en el dispositivo ALSA `default`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// (L+R)/2 en los dos canales: el parlante solo reproduce el canal izquierdo.
    Mono,
    /// Sin mezcla, para auriculares.
    Stereo,
}

impl Mode {
    pub fn toggled(self) -> Mode {
        match self {
            Mode::Mono => Mode::Stereo,
            Mode::Stereo => Mode::Mono,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Mono => "mono",
            Mode::Stereo => "stereo",
        }
    }

    /// Nombre para la pantalla.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Mono => "MONO",
            Mode::Stereo => "ESTÉREO",
        }
    }

    /// Modo que corresponde al estado del jack: auriculares → estéreo; parlante → mono.
    pub fn for_headphones(inserted: bool) -> Mode {
        if inserted { Mode::Stereo } else { Mode::Mono }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Mode, String> {
        match s.trim() {
            "mono" => Ok(Mode::Mono),
            "stereo" => Ok(Mode::Stereo),
            other => Err(format!("modo desconocido: {other:?} (mono|stereo)")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioState {
    pub volume_pct: u8,
    pub volume_db: Option<f64>,
    pub mode: Mode,
    pub seq: u64,
}

impl AudioState {
    pub fn render(&self) -> String {
        let mut out = format!("volume_pct={}\n", self.volume_pct);
        if let Some(db) = self.volume_db {
            out += &format!("volume_db={db:.1}\n");
        }
        out += &format!("mode={}\nseq={}\n", self.mode, self.seq);
        out
    }

    pub fn parse(text: &str) -> Result<AudioState, String> {
        let (mut pct, mut db, mut mode, mut seq) = (None, None, None, 0);
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("línea sin '=': {line:?}"));
            };
            let value = value.trim();
            let bad = |what: &str| format!("{what} inválido: {value:?}");
            match key.trim() {
                "volume_pct" => {
                    let v: u8 = value.parse().map_err(|_| bad("volume_pct"))?;
                    pct = Some(v.min(100));
                }
                "volume_db" => db = Some(value.parse().map_err(|_| bad("volume_db"))?),
                "mode" => mode = Some(value.parse()?),
                "seq" => seq = value.parse().map_err(|_| bad("seq"))?,
                _ => {}
            }
        }
        Ok(AudioState {
            volume_pct: pct.ok_or("falta volume_pct")?,
            volume_db: db,
            mode: mode.ok_or("falta mode")?,
            seq,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parse_and_print_round_trip() {
        for mode in [Mode::Mono, Mode::Stereo] {
            assert_eq!(mode.as_str().parse::<Mode>(), Ok(mode));
        }
        assert_eq!("stereo\n".parse::<Mode>(), Ok(Mode::Stereo));
        assert!("estéreo".parse::<Mode>().is_err());
        assert_eq!(Mode::Stereo.label(), "ESTÉREO");
    }

    #[test]
    fn mode_toggle_and_jack() {
        assert_eq!(Mode::Mono.toggled(), Mode::Stereo);
        assert_eq!(Mode::Stereo.toggled(), Mode::Mono);
        assert_eq!(Mode::for_headphones(true), Mode::Stereo);
        assert_eq!(Mode::for_headphones(false), Mode::Mono);
    }

    #[test]
    fn percent_is_linear_in_db() {
        assert_eq!(volume_percent(0.0, 0.0), 100);
        assert_eq!(volume_percent(-60.0, 0.0), 0);
        assert_eq!(volume_percent(-95.0, 0.0), 0);
        assert_eq!(volume_percent(-30.0, 0.0), 50);
        // Un paso de 3 dB = 5 %.
        assert_eq!(volume_percent(-27.0, 0.0), 55);
        // RK817: 201 crudo ≈ -20,12 dB.
        assert_eq!(volume_percent(-20.12, 0.0), 66);
        // Controles con máximo distinto de 0 dB.
        assert_eq!(volume_percent(6.0, 6.0), 100);
    }

    #[test]
    fn state_round_trip() {
        let s = AudioState {
            volume_pct: 66,
            volume_db: Some(-20.12),
            mode: Mode::Mono,
            seq: 3,
        };
        let text = s.render();
        assert_eq!(text, "volume_pct=66\nvolume_db=-20.1\nmode=mono\nseq=3\n");
        let back = AudioState::parse(&text).unwrap();
        assert_eq!(back.volume_db, Some(-20.1));
        assert_eq!(
            AudioState {
                volume_db: s.volume_db,
                ..back
            },
            s
        );
    }

    #[test]
    fn parse_is_tolerant_with_extras_and_strict_with_required() {
        let s = AudioState::parse("# x\n\nfuture=1\n mode = stereo \nvolume_pct=150\n").unwrap();
        assert_eq!(s.mode, Mode::Stereo);
        assert_eq!(s.volume_pct, 100);
        assert_eq!(s.seq, 0);
        assert_eq!(s.volume_db, None);
        assert!(AudioState::parse("mode=mono\n").is_err());
        assert!(AudioState::parse("volume_pct=5\n").is_err());
        assert!(AudioState::parse("volume_pct=5\nmode=surround\n").is_err());
        assert!(AudioState::parse("volume_pct=x\nmode=mono\n").is_err());
        assert!(AudioState::parse("volume_pct=5\nmode=mono\nbasura\n").is_err());
        assert!(AudioState::parse("").is_err());
    }
}
