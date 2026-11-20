//! Percusión sintetizada: bombo, caja, hats y palmas (recetas en dsp.md).
//!
//! `DrumSetup` guarda los parámetros de la pista ya convertidos (se calcula al recibir el
//! instrumento); `DrumVoice` es una nota sonando. Cada modelo tiene su propio bucle para que
//! el `match` quede fuera del bucle por muestra.

use super::dsp::{Rng, SILENCE, Svf, decay_coef, sin_turns};
use crate::instrument::{DRUM_ROOT_NOTE, DrumModel, DrumParams, env_seconds, tune_semitones};

/// Los decaimientos auxiliares (barrido, click) se fuerzan a 0 por debajo de esto, una vez por
/// bloque: así nunca llegan a subnormales aunque no haya flush-to-zero.
const TINY: f32 = 1.0e-6;
/// Duración del click de ataque (hasta −60 dB).
const CLICK_SECONDS: f32 = 0.004;

// Niveles relativos de cada modelo, ajustados para picos de ~0,5–0,9 con velocidad 127
// (medidos con el render offline: `groovebox render`).
const KICK_LEVEL: f32 = 0.9;
const CLICK_LEVEL: f32 = 0.35;
const SNARE_TONE_LEVEL: f32 = 0.7;
const SNARE_NOISE_LEVEL: f32 = 0.55;
const HAT_LEVEL: f32 = 0.45;
const CLAP_LEVEL: f32 = 1.1;

#[derive(Clone, Copy, Debug)]
pub struct DrumSetup {
    model: DrumModel,
    /// Frecuencia nominal (tono o filtro) en Hz, con la afinación aplicada.
    base_hz: f32,
    decay_s: f32,
    sweep_ratio: f32,
    sweep_s: f32,
    color: f32,
    punch: f32,
    spread_s: f32,
    sample_rate: f32,
}

impl DrumSetup {
    pub fn new(p: &DrumParams, sample_rate: f32) -> Self {
        let nominal = match p.model {
            DrumModel::Kick => 48.0,
            DrumModel::Snare => 185.0,
            DrumModel::ClosedHat | DrumModel::OpenHat => 7500.0,
            DrumModel::Clap => 1200.0,
        };
        let unit = |v: u8| f32::from(v.min(127)) / 127.0;
        Self {
            model: p.model,
            base_hz: nominal * (tune_semitones(p.tune) / 12.0).exp2(),
            decay_s: env_seconds(p.decay),
            sweep_ratio: (3.0 * unit(p.sweep)).exp2(),
            sweep_s: env_seconds(p.sweep_time),
            color: unit(p.color),
            punch: unit(p.punch),
            spread_s: 0.004 + 0.016 * unit(p.spread),
            sample_rate,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DrumVoice {
    model: DrumModel,
    gain: f32,
    /// Envolvente principal (cuerpo o cola) y su coeficiente por muestra.
    amp: f32,
    amp_k: f32,
    /// Oscilador con barrido exponencial: f = end + span·sweep (ciclos por muestra).
    phase: f32,
    freq_end: f32,
    freq_span: f32,
    sweep: f32,
    sweep_k: f32,
    tone_gain: f32,
    /// Envolvente del ruido (caja).
    noise: f32,
    noise_k: f32,
    noise_gain: f32,
    click: f32,
    click_k: f32,
    rng: Rng,
    svf: Svf,
    /// Palmas: ráfagas que faltan, muestras hasta la próxima y coeficiente de la cola.
    bursts_left: u8,
    burst_timer: u32,
    spread: u32,
    tail_k: f32,
}

impl DrumVoice {
    pub fn new(s: &DrumSetup, note: u8, gain: f32, seed: u32) -> Self {
        let sr = s.sample_rate;
        let pitch = ((f32::from(note) - f32::from(DRUM_ROOT_NOTE)) / 12.0).exp2();
        let hz = (s.base_hz * pitch).min(0.45 * sr);
        let mut v = Self {
            model: s.model,
            gain,
            amp: 1.0,
            amp_k: decay_coef(s.decay_s, sr),
            phase: 0.0,
            freq_end: hz / sr,
            freq_span: 0.0,
            sweep: 0.0,
            sweep_k: 0.0,
            tone_gain: 0.0,
            noise: 0.0,
            noise_k: 0.0,
            noise_gain: 0.0,
            click: CLICK_LEVEL * s.punch,
            click_k: decay_coef(CLICK_SECONDS, sr),
            rng: Rng::new(seed),
            svf: Svf::default(),
            bursts_left: 0,
            burst_timer: 0,
            spread: 0,
            tail_k: 0.0,
        };
        match s.model {
            DrumModel::Kick => {
                v.freq_span = v.freq_end * (s.sweep_ratio - 1.0);
                v.sweep = 1.0;
                v.sweep_k = decay_coef(s.sweep_s, sr);
                v.tone_gain = KICK_LEVEL;
            }
            DrumModel::Snare => {
                // Cuerpo: seno que cae media octava en ~15 ms y dura la mitad que el ruido.
                v.freq_span = v.freq_end * 0.5;
                v.sweep = 1.0;
                v.sweep_k = decay_coef(0.015, sr);
                v.amp_k = decay_coef(0.5 * s.decay_s, sr);
                v.tone_gain = SNARE_TONE_LEVEL * (1.0 - s.color);
                v.noise = 1.0;
                v.noise_k = decay_coef(s.decay_s, sr);
                v.noise_gain = SNARE_NOISE_LEVEL * s.color;
                // Ruido por un pasa-altos/banda alrededor de 1,8 kHz (sigue la afinación).
                v.svf.set(hz * 10.0, 1.2, sr);
            }
            DrumModel::ClosedHat | DrumModel::OpenHat => {
                v.click = 0.0;
                v.tone_gain = HAT_LEVEL;
                // Color = resonancia: de k 1,6 (opaco) a 0,4 (metálico).
                v.svf.set(hz, 1.6 - 1.2 * s.color, sr);
            }
            DrumModel::Clap => {
                v.click = 0.0;
                let k = 1.4 - 0.8 * s.color;
                v.svf.set(hz, k, sr);
                // Pasa-banda normalizado (k·band) para que el color no cambie el nivel.
                v.tone_gain = CLAP_LEVEL * k;
                v.spread = (s.spread_s * sr).max(1.0) as u32;
                v.burst_timer = v.spread;
                v.bursts_left = 3;
                // Cada ráfaga cae ~30 dB antes de la siguiente.
                v.amp_k = decay_coef(2.0 * s.spread_s, sr);
                v.tail_k = decay_coef(s.decay_s, sr);
            }
        }
        v
    }

    /// Suma la voz en `out`. Devuelve false cuando terminó.
    pub fn render(&mut self, out: &mut [f32]) -> bool {
        let alive = match self.model {
            DrumModel::Kick => self.render_kick(out),
            DrumModel::Snare => self.render_snare(out),
            DrumModel::ClosedHat | DrumModel::OpenHat => self.render_hat(out),
            DrumModel::Clap => self.render_clap(out),
        };
        for x in [&mut self.sweep, &mut self.click, &mut self.noise] {
            if *x < TINY {
                *x = 0.0;
            }
        }
        alive
    }

    #[inline]
    fn advance_tone(&mut self) -> f32 {
        self.phase += self.freq_end + self.freq_span * self.sweep;
        self.sweep *= self.sweep_k;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        sin_turns(self.phase)
    }

    /// ~25 operaciones por muestra.
    fn render_kick(&mut self, out: &mut [f32]) -> bool {
        for o in out.iter_mut() {
            let tone = self.advance_tone() * self.amp * self.tone_gain;
            let click = self.rng.bipolar() * self.click;
            self.amp *= self.amp_k;
            self.click *= self.click_k;
            *o += (tone + click) * self.gain;
        }
        self.amp > SILENCE
    }

    /// ~40 operaciones por muestra (seno + ruido + SVF).
    fn render_snare(&mut self, out: &mut [f32]) -> bool {
        for o in out.iter_mut() {
            let tone = self.advance_tone() * self.amp * self.tone_gain;
            // La envolvente va después del filtro: su estado no decae hacia subnormales.
            let f = self.svf.process(self.rng.bipolar());
            let noise = (f.high + 0.5 * f.band) * self.noise * self.noise_gain;
            let click = self.rng.bipolar() * self.click;
            self.amp *= self.amp_k;
            self.noise *= self.noise_k;
            self.click *= self.click_k;
            *o += (tone + noise + click) * self.gain;
        }
        self.amp > SILENCE || self.noise > SILENCE
    }

    /// ~20 operaciones por muestra.
    fn render_hat(&mut self, out: &mut [f32]) -> bool {
        for o in out.iter_mut() {
            let f = self.svf.process(self.rng.bipolar());
            *o += (f.high + 0.5 * f.band) * self.amp * self.tone_gain * self.gain;
            self.amp *= self.amp_k;
        }
        self.amp > SILENCE
    }

    /// ~22 operaciones por muestra: 3 ráfagas cortas y una cola, ruido por pasa-banda.
    fn render_clap(&mut self, out: &mut [f32]) -> bool {
        for o in out.iter_mut() {
            if self.bursts_left > 0 {
                self.burst_timer -= 1;
                if self.burst_timer == 0 {
                    self.bursts_left -= 1;
                    self.amp = 1.0;
                    self.burst_timer = self.spread;
                    if self.bursts_left == 0 {
                        self.amp_k = self.tail_k;
                    }
                }
            }
            let f = self.svf.process(self.rng.bipolar());
            *o += f.band * self.amp * self.tone_gain * self.gain;
            self.amp *= self.amp_k;
        }
        self.bursts_left > 0 || self.amp > SILENCE
    }
}
