//! Mezcla (H9): ajustes de los efectos de envío y del master tal como los edita la interfaz y
//! los guarda el proyecto (todo en `u8` 0–127 salvo la ganancia master en dB), y las leyes que
//! los pasan a ganancias. El motor convierte al recibir el comando, nunca por muestra.
//! Diseño y fuentes en docs/solutions/groovebox/dsp.md, "Mezcla, efectos y master (H9)".

use serde::{Deserialize, Serialize};

/// Ganancia de un nivel 0–127 (volumen de pista, envíos, retornos): ley cuadrática con
/// 100 = 0 dB (127 = +4,2 dB, 50 = −12 dB, 0 = silencio). Es la curva habitual de un fader
/// barato: fina arriba y que llega a silencio sin tabla.
pub fn level_gain(v: u8) -> f32 {
    level_gain_f(f32::from(v))
}

/// `level_gain` con un nivel fraccionario (el valor guardado más el stick), recortado a 0–127.
pub fn level_gain_f(v: f32) -> f32 {
    let x = v.clamp(0.0, 127.0) / 100.0;
    x * x
}

/// `level_gain` en dB, para mostrarlo (None = silencio).
pub fn level_db(v: u8) -> Option<f32> {
    (v > 0).then(|| 20.0 * level_gain(v).log10())
}

/// Paneo −64..=63 a ganancias (L, R): ley seno/coseno de potencia constante (L² + R² = 2),
/// normalizada a 0 dB en el centro para que una pista centrada suene igual que antes del H9.
/// En un extremo, un canal queda en +3 dB y el otro en silencio.
pub fn pan_gains(pan: i8) -> (f32, f32) {
    if pan == 0 {
        return (1.0, 1.0);
    }
    let p = (f32::from(pan) / 63.0).clamp(-1.0, 1.0);
    let theta = (p + 1.0) * std::f32::consts::FRAC_PI_4;
    let (s, c) = theta.sin_cos();
    (std::f32::consts::SQRT_2 * c, std::f32::consts::SQRT_2 * s)
}

/// Texto del paneo como en el mockup de spec-ui: `C`, `L20`, `R15`.
pub fn pan_label(pan: i8) -> String {
    match pan {
        0 => "C".to_string(),
        p if p < 0 => format!("L{}", -i16::from(p)),
        p => format!("R{p}"),
    }
}

/// Tiempo del delay en fracciones de compás (spec-motor, "Efectos"). Los nombres de las
/// variantes son parte del formato de archivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DelayTime {
    Sixteenth,
    Eighth,
    #[default]
    Sixteenth3,
    Quarter,
    Eighth3,
    Half,
    Bar,
}

impl DelayTime {
    pub const ALL: [DelayTime; 7] = [
        DelayTime::Sixteenth,
        DelayTime::Eighth,
        DelayTime::Sixteenth3,
        DelayTime::Quarter,
        DelayTime::Eighth3,
        DelayTime::Half,
        DelayTime::Bar,
    ];

    /// Duración en semicorcheas (pasos del secuenciador).
    pub fn steps(self) -> u32 {
        match self {
            DelayTime::Sixteenth => 1,
            DelayTime::Eighth => 2,
            DelayTime::Sixteenth3 => 3,
            DelayTime::Quarter => 4,
            DelayTime::Eighth3 => 6,
            DelayTime::Half => 8,
            DelayTime::Bar => 16,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DelayTime::Sixteenth => "1/16",
            DelayTime::Eighth => "1/8",
            DelayTime::Sixteenth3 => "3/16",
            DelayTime::Quarter => "1/4",
            DelayTime::Eighth3 => "3/8",
            DelayTime::Half => "1/2",
            DelayTime::Bar => "1 comp",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(2)
    }

    pub fn seconds(self, bpm: f32) -> f32 {
        self.steps() as f32 * 15.0 / bpm
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DelaySettings {
    pub time: DelayTime,
    /// 0–127 → realimentación 0–95 %.
    pub feedback: u8,
    /// Corte del pasa-bajos de la realimentación: 0 = 500 Hz, 127 = 16 kHz.
    pub tone: u8,
    /// 0 = ecos estéreo en su lugar, 127 = ping-pong completo.
    pub width: u8,
    pub return_level: u8,
}

impl Default for DelaySettings {
    /// Los valores del mockup de spec-ui (Mezcla).
    fn default() -> Self {
        Self {
            time: DelayTime::Sixteenth3,
            feedback: 40,
            tone: 90,
            width: 100,
            return_level: 80,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReverbSettings {
    /// Largo de la cola (RT60): 0 = 0,3 s, 127 = 8 s, exponencial.
    pub size: u8,
    /// Amortiguación de los agudos en la realimentación: 0 = brillante, 127 = oscura.
    pub damping: u8,
    /// 0–127 → 0–100 ms.
    pub predelay: u8,
    pub return_level: u8,
}

impl Default for ReverbSettings {
    fn default() -> Self {
        Self {
            size: 60,
            damping: 50,
            predelay: 10,
            return_level: 70,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FxSettings {
    pub delay: DelaySettings,
    pub reverb: ReverbSettings,
}

pub const MASTER_GAIN_MIN: i8 = -24;
pub const MASTER_GAIN_MAX: i8 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MasterSettings {
    /// Ganancia en dB, −24..=+12.
    pub gain: i8,
    /// Limitador de picos antes del soft clip (ver dsp.md).
    pub limiter: bool,
}

impl Default for MasterSettings {
    fn default() -> Self {
        Self {
            gain: 0,
            limiter: true,
        }
    }
}

impl MasterSettings {
    pub fn gain_linear(&self) -> f32 {
        let db = self.gain.clamp(MASTER_GAIN_MIN, MASTER_GAIN_MAX);
        if db == 0 {
            1.0
        } else {
            10f32.powf(f32::from(db) / 20.0)
        }
    }
}

// Mapeos de los efectos (los usan el motor y la pantalla Mezcla para mostrar unidades).

fn unit(v: u8) -> f32 {
    f32::from(v.min(127)) / 127.0
}

pub fn delay_feedback(v: u8) -> f32 {
    0.95 * unit(v)
}

pub fn delay_tone_hz(v: u8) -> f32 {
    500.0 * 32f32.powf(unit(v))
}

pub fn reverb_rt60(v: u8) -> f32 {
    0.3 * (8.0f32 / 0.3).powf(unit(v))
}

pub fn reverb_damping_hz(v: u8) -> f32 {
    // 127 = 1,5 kHz (oscura), 0 = 18 kHz (casi sin amortiguar).
    18_000.0 * (1500.0f32 / 18_000.0).powf(unit(v))
}

pub fn reverb_predelay_s(v: u8) -> f32 {
    0.1 * unit(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_law_has_unity_at_100() {
        assert_eq!(level_gain(100), 1.0);
        assert_eq!(level_gain(0), 0.0);
        assert!((level_db(50).unwrap() + 12.04).abs() < 0.01);
        assert!((level_db(127).unwrap() - 4.15).abs() < 0.01);
    }

    #[test]
    fn pan_law_is_constant_power_with_unity_center() {
        assert_eq!(pan_gains(0), (1.0, 1.0));
        for pan in -64..=63i8 {
            let (l, r) = pan_gains(pan);
            assert!((l * l + r * r - 2.0).abs() < 1e-5, "pan {pan}");
        }
        let (l, r) = pan_gains(-64);
        assert!((l - std::f32::consts::SQRT_2).abs() < 1e-6 && r.abs() < 1e-6);
        let (l, r) = pan_gains(63);
        assert!(l.abs() < 1e-6 && (r - std::f32::consts::SQRT_2).abs() < 1e-6);
        // Monótono: a la derecha, L baja y R sube.
        assert!(pan_gains(10).0 < pan_gains(5).0 && pan_gains(10).1 > pan_gains(5).1);
        assert_eq!(pan_label(-20), "L20");
        assert_eq!(pan_label(15), "R15");
    }

    #[test]
    fn delay_times_follow_the_tempo() {
        assert!((DelayTime::Sixteenth3.seconds(120.0) - 0.375).abs() < 1e-6);
        assert!((DelayTime::Bar.seconds(120.0) - 2.0).abs() < 1e-6);
        for (i, t) in DelayTime::ALL.iter().enumerate() {
            assert_eq!(t.index(), i);
        }
    }

    #[test]
    fn fx_mappings_cover_their_ranges() {
        assert!((reverb_rt60(0) - 0.3).abs() < 1e-4 && (reverb_rt60(127) - 8.0).abs() < 1e-3);
        assert!((delay_tone_hz(127) - 16_000.0).abs() < 1.0);
        assert!(delay_feedback(127) < 0.951);
        assert_eq!(MasterSettings::default().gain_linear(), 1.0);
    }
}
