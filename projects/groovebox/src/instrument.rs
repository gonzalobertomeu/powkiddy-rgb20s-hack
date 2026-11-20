//! Parámetros de los instrumentos tal como los edita la interfaz y los guarda el proyecto:
//! todo en `u8` 0–127 (spec-motor, "Instrumentos"). El motor los pasa a valores físicos al
//! recibirlos (`engine::drum::DrumSetup`, `engine::synth::SynthSetup`), nunca por muestra.

use serde::{Deserialize, Serialize};

use crate::model::TrackKind;

pub const MAX_VOICES: u8 = 8;
/// Nota en la que la percusión suena con su afinación nominal (C-4).
pub const DRUM_ROOT_NOTE: u8 = 60;

/// Los nombres de las variantes de `DrumModel` y `Wave` son parte del formato de archivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrumModel {
    Kick,
    Snare,
    ClosedHat,
    OpenHat,
    Clap,
}

/// Parámetros comunes a los modelos de percusión; cada modelo usa los suyos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrumParams {
    pub model: DrumModel,
    /// 64 = afinación nominal; ±24 semitonos en los extremos.
    pub tune: u8,
    /// Tiempo hasta −60 dB (mapeo de tiempos de envolvente).
    pub decay: u8,
    /// Bombo: profundidad del barrido de pitch (hasta 3 octavas).
    pub sweep: u8,
    /// Bombo: tiempo del barrido (mapeo de tiempos de envolvente).
    pub sweep_time: u8,
    /// Caja: proporción de ruido. Hats y palmas: brillo (resonancia del filtro).
    pub color: u8,
    /// Nivel del click de ataque (bombo y caja).
    pub punch: u8,
    /// Palmas: separación entre ráfagas (4–20 ms).
    pub spread: u8,
}

impl DrumParams {
    pub fn new(model: DrumModel) -> Self {
        let base = Self {
            model,
            tune: 64,
            decay: 60,
            sweep: 0,
            sweep_time: 50,
            color: 64,
            punch: 0,
            spread: 48,
        };
        match model {
            // 450 ms de caída, barrido ×3,5 en 40 ms.
            DrumModel::Kick => Self {
                decay: 84,
                sweep: 76,
                sweep_time: 51,
                punch: 50,
                ..base
            },
            // 220 ms, 55 % de ruido.
            DrumModel::Snare => Self {
                decay: 74,
                color: 70,
                punch: 40,
                ..base
            },
            DrumModel::ClosedHat => Self { decay: 56, ..base },
            DrumModel::OpenHat => Self { decay: 84, ..base },
            // 250 ms de cola, ráfagas cada 10 ms.
            DrumModel::Clap => Self { decay: 76, ..base },
        }
    }
}

/// `Square` y `Sine` solo llegan desde un archivo hasta la pantalla Instrumento (H8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wave {
    Saw,
    Square,
    Sine,
}

/// Voz sustractiva mínima de H2: oscilador PolyBLEP, pasa-bajos SVF con envolvente de decaimiento
/// y ADSR de amplitud. El resto (sub, ruido, ancho de pulso, LFO) llega en H8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SynthParams {
    pub wave: Wave,
    pub attack: u8,
    pub decay: u8,
    /// Nivel de sostenido, lineal 0–127.
    pub sustain: u8,
    pub release: u8,
    pub cutoff: u8,
    pub resonance: u8,
    /// Cantidad de la envolvente de filtro: 64 = nada, ±6 octavas en los extremos.
    pub filter_env: u8,
    pub filter_decay: u8,
}

impl SynthParams {
    /// Bajo: sierra, ataque corto, filtro cerrado con un "pluck" de 3 octavas.
    pub fn bass() -> Self {
        Self {
            wave: Wave::Saw,
            attack: 10,
            decay: 79,
            sustain: 50,
            release: 60,
            cutoff: 46,
            resonance: 40,
            filter_env: 96,
            filter_decay: 72,
        }
    }

    /// Sinte polifónico para acordes: sierra, ataque suave y cola larga.
    pub fn poly() -> Self {
        Self {
            wave: Wave::Saw,
            attack: 37,
            decay: 92,
            sustain: 80,
            release: 83,
            cutoff: 79,
            resonance: 20,
            filter_env: 80,
            filter_decay: 86,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Patch {
    Drum(DrumParams),
    /// Sin sample cargado (H7): no suena.
    Sampler,
    Synth(SynthParams),
}

/// Instrumento de una pista: el patch más la sección GENERAL de la spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instrument {
    pub patch: Patch,
    pub volume: u8,
    /// Cuánto afecta la velocidad a la amplitud (127 = todo).
    pub velocity_amount: u8,
    /// Polifonía de la pista, 1–8 (ADR 0007).
    pub voices: u8,
}

impl Instrument {
    pub fn new(patch: Patch, voices: u8) -> Self {
        Self {
            patch,
            volume: 110,
            velocity_amount: 127,
            voices: voices.clamp(1, MAX_VOICES),
        }
    }

    pub fn kind(&self) -> TrackKind {
        match self.patch {
            Patch::Drum(_) => TrackKind::Drum,
            Patch::Sampler => TrackKind::Sample,
            Patch::Synth(_) => TrackKind::Synth,
        }
    }
}

/// Voces por defecto del instrumento de `track` (spec-motor, "Voces y polifonía"): las del kit si
/// la pista sigue con el mismo tipo; si no, las del tipo (percusión 1, sampler 2, sustractivo 4).
pub fn default_voices(track: usize, kind: TrackKind) -> u8 {
    match default_kit().get(track) {
        Some((_, kit)) if kit.kind() == kind => kit.voices,
        _ => match kind {
            TrackKind::Drum => 1,
            TrackKind::Sample => 2,
            TrackKind::Synth => 4,
        },
    }
}

/// Kit del proyecto nuevo (spec-motor, "Modelo de datos").
pub fn default_kit() -> [(&'static str, Instrument); 8] {
    let drum = |m| Instrument::new(Patch::Drum(DrumParams::new(m)), 1);
    [
        ("BOMBO", drum(DrumModel::Kick)),
        ("CAJA", drum(DrumModel::Snare)),
        ("HH-CER", drum(DrumModel::ClosedHat)),
        ("HH-ABI", drum(DrumModel::OpenHat)),
        ("PALMAS", drum(DrumModel::Clap)),
        ("SAMPLE", Instrument::new(Patch::Sampler, 2)),
        (
            "BAJO",
            Instrument::new(Patch::Synth(SynthParams::bass()), 1),
        ),
        (
            "SINTE",
            Instrument::new(Patch::Synth(SynthParams::poly()), 4),
        ),
    ]
}

// Mapeos de 0–127 a valores físicos (tabla "Mapeo" de spec-motor).

fn unit(v: u8) -> f32 {
    f32::from(v.min(127)) / 127.0
}

/// 1 ms a 10 s, exponencial.
pub fn env_seconds(v: u8) -> f32 {
    0.001 * 10_000f32.powf(unit(v))
}

/// 20 Hz a 20 kHz, exponencial.
pub fn cutoff_hz(v: u8) -> f32 {
    20.0 * 1000f32.powf(unit(v))
}

/// Amortiguamiento `k` del SVF: 2 (sin resonancia) a 0,05 (casi autooscilación).
pub fn resonance_k(v: u8) -> f32 {
    2.0 - 1.95 * unit(v)
}

/// −60 a +6 dB; 0 = silencio.
pub fn volume_gain(v: u8) -> f32 {
    if v == 0 {
        0.0
    } else {
        10f32.powf((-60.0 + 66.0 * unit(v)) / 20.0)
    }
}

/// Valores centrados en 64 a −1..=1.
pub fn bipolar(v: u8) -> f32 {
    ((f32::from(v.min(127)) - 64.0) / 63.0).clamp(-1.0, 1.0)
}

pub fn tune_semitones(v: u8) -> f32 {
    24.0 * bipolar(v)
}

/// Ganancia por velocidad: lineal en amplitud, con `amount` de 0 (ignora la velocidad) a 127.
pub fn velocity_gain(velocity: u8, amount: u8) -> f32 {
    let a = unit(amount);
    1.0 - a + a * unit(velocity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings_match_spec_table() {
        assert!((env_seconds(0) - 0.001).abs() < 1e-6);
        assert!((env_seconds(127) - 10.0).abs() < 1e-3);
        assert!((env_seconds(40) - 0.018).abs() < 0.001);
        assert!((cutoff_hz(64) - 650.0).abs() < 25.0);
        assert!((cutoff_hz(127) - 20_000.0).abs() < 1.0);
        assert_eq!(volume_gain(0), 0.0);
        let db = |v| 20.0 * volume_gain(v).log10();
        assert!((db(100) + 8.0).abs() < 0.1);
        assert!((db(127) - 6.0).abs() < 0.01);
        assert_eq!(bipolar(64), 0.0);
        assert_eq!(bipolar(127), 1.0);
    }

    #[test]
    fn default_kit_kinds_and_voices() {
        let kit = default_kit();
        assert_eq!(kit[0].1.kind(), TrackKind::Drum);
        assert_eq!(kit[5].1.kind(), TrackKind::Sample);
        assert_eq!(kit[6].1.voices, 1);
        assert_eq!(kit[7].1.voices, 4);
    }
}
