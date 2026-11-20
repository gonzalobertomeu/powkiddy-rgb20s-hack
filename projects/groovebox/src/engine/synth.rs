//! Voz sustractiva mínima: oscilador PolyBLEP → SVF pasa-bajos (con envolvente de decaimiento y
//! seguimiento de teclado) → ADSR exponencial. El corte se recalcula una vez por llamada a
//! `render` (≤ 32 muestras), no por muestra: `exp2` + `tan` amortizados.

use super::dsp::{SILENCE, Svf, one_pole_coef, poly_blep, sin_turns};
use crate::instrument::{SynthParams, Wave, bipolar, cutoff_hz, env_seconds, resonance_k};

/// Nivel de una voz a velocidad 127: un acorde de 4 notas queda cerca de 1 antes del master.
pub const VOICE_LEVEL: f32 = 0.35;
/// Seguimiento de teclado fijo del 50 % (en octavas), referido a C-4. Parámetro en H8.
const KEY_TRACK: f32 = 0.5;
const KEY_TRACK_REF_HZ: f32 = 261.63;
/// En el ataque se apunta a 1,2 y se corta en 1: la curva no se aplana al final.
const ATTACK_TARGET: f32 = 1.2;
/// Suavizado de la ganancia de la voz al cambiar de velocidad en legato.
const GAIN_TAU: f32 = 0.002;
const TINY: f32 = 1.0e-6;

pub fn midi_to_hz(note: u8) -> f32 {
    440.0 * ((f32::from(note) - 69.0) / 12.0).exp2()
}

#[derive(Clone, Copy, Debug)]
pub struct SynthSetup {
    wave: Wave,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    cutoff_hz: f32,
    k: f32,
    env_octaves: f32,
    filter_k: f32,
    gain_coef: f32,
    sample_rate: f32,
}

impl SynthSetup {
    pub fn new(p: &SynthParams, sample_rate: f32) -> Self {
        let sr = sample_rate;
        // Constantes de tiempo para que el ataque llegue a 1 y la caída y la relajación lleguen
        // a −60 dB en el tiempo del parámetro (ver dsp.md, "Envolventes").
        let attack_tau = env_seconds(p.attack) / (ATTACK_TARGET / (ATTACK_TARGET - 1.0)).ln();
        let t60 = |v| env_seconds(v) / 1000f32.ln();
        Self {
            wave: p.wave,
            attack: one_pole_coef(attack_tau, sr),
            decay: one_pole_coef(t60(p.decay), sr),
            sustain: f32::from(p.sustain.min(127)) / 127.0,
            release: one_pole_coef(t60(p.release), sr),
            cutoff_hz: cutoff_hz(p.cutoff),
            k: resonance_k(p.resonance),
            env_octaves: 6.0 * bipolar(p.filter_env),
            filter_k: super::dsp::decay_coef(env_seconds(p.filter_decay), sr),
            gain_coef: one_pole_coef(GAIN_TAU, sr),
            sample_rate,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Attack,
    Decay,
    Release,
    Off,
}

/// ADSR exponencial (one-pole hacia un objetivo). Redisparar o soltar parte del nivel actual:
/// nunca salta.
#[derive(Clone, Copy, Debug)]
struct Adsr {
    stage: Stage,
    value: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SynthVoice {
    setup: SynthSetup,
    phase: f32,
    dt: f32,
    gain: f32,
    gain_target: f32,
    env: Adsr,
    filter_env: f32,
    /// Corte sin envolvente, con el seguimiento de teclado.
    cutoff: f32,
    svf: Svf,
}

impl SynthVoice {
    pub fn new(setup: &SynthSetup, note: u8, gain: f32) -> Self {
        let mut v = Self {
            setup: *setup,
            phase: 0.0,
            dt: 0.0,
            gain: gain * VOICE_LEVEL,
            gain_target: gain * VOICE_LEVEL,
            env: Adsr {
                stage: Stage::Attack,
                value: 0.0,
            },
            filter_env: 1.0,
            cutoff: 0.0,
            svf: Svf::default(),
        };
        v.set_note(note);
        v
    }

    fn set_note(&mut self, note: u8) {
        let hz = midi_to_hz(note);
        self.dt = (hz / self.setup.sample_rate).min(0.45);
        self.cutoff = self.setup.cutoff_hz * (hz / KEY_TRACK_REF_HZ).powf(KEY_TRACK);
    }

    /// Modo mono: nueva nota sin cortar la anterior. La fase sigue y la envolvente sube desde
    /// su nivel actual.
    pub fn legato(&mut self, setup: &SynthSetup, note: u8, gain: f32) {
        self.setup = *setup;
        self.set_note(note);
        self.gain_target = gain * VOICE_LEVEL;
        self.env.stage = Stage::Attack;
        self.filter_env = 1.0;
    }

    /// Amplitud actual aproximada (envolvente × ganancia).
    pub fn level(&self) -> f32 {
        self.env.value * self.gain
    }

    pub fn release(&mut self) {
        if self.env.stage != Stage::Off {
            self.env.stage = Stage::Release;
        }
    }

    /// Suma la voz en `out`. Devuelve false cuando terminó la relajación.
    pub fn render(&mut self, out: &mut [f32]) -> bool {
        if self.env.stage == Stage::Off {
            return false;
        }
        let s = self.setup;
        let cutoff = self.cutoff * (s.env_octaves * self.filter_env).exp2();
        self.svf.set(cutoff, s.k, s.sample_rate);
        self.filter_env *= s.filter_k.powi(out.len() as i32);
        if self.filter_env < TINY {
            self.filter_env = 0.0;
        }
        match s.wave {
            Wave::Saw => self.run(out, |p, dt| 2.0 * p - 1.0 - poly_blep(p, dt)),
            Wave::Square => self.run(out, |p, dt| {
                let naive = if p < 0.5 { 1.0 } else { -1.0 };
                let p2 = if p < 0.5 { p + 0.5 } else { p - 0.5 };
                naive + poly_blep(p, dt) - poly_blep(p2, dt)
            }),
            Wave::Sine => self.run(out, |p, _| sin_turns(p)),
        }
        self.env.stage != Stage::Off
    }

    /// ~45 operaciones por muestra con sierra: oscilador ~10, SVF ~10, ADSR ~5, ganancia ~4.
    #[inline(always)]
    fn run(&mut self, out: &mut [f32], osc: impl Fn(f32, f32) -> f32) {
        let s = self.setup;
        for o in out.iter_mut() {
            let x = osc(self.phase, self.dt);
            self.phase += self.dt;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            let y = self.svf.process(x).low;
            let env = &mut self.env;
            match env.stage {
                Stage::Attack => {
                    env.value += (ATTACK_TARGET - env.value) * s.attack;
                    if env.value >= 1.0 {
                        env.value = 1.0;
                        env.stage = Stage::Decay;
                    }
                }
                Stage::Decay => {
                    env.value += (s.sustain - env.value) * s.decay;
                    if (env.value - s.sustain).abs() < TINY {
                        env.value = s.sustain;
                    }
                }
                Stage::Release => {
                    env.value -= env.value * s.release;
                    if env.value < SILENCE {
                        env.value = 0.0;
                        env.stage = Stage::Off;
                    }
                }
                Stage::Off => {}
            }
            self.gain += (self.gain_target - self.gain) * s.gain_coef;
            *o += y * env.value * self.gain;
        }
    }
}
