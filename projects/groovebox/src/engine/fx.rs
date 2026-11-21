//! Mezcla y efectos de envío (H9): rampas de parámetros por bloque, delay estéreo sincronizado
//! al tempo, reverb FDN de 4 líneas, limitador de picos del master y el filtro de bus que mueve
//! el stick izquierdo en las pistas de percusión.
//!
//! Todo se preasigna en `new` (fuera del hilo de audio); `process` solo hace aritmética.
//! Diseño, costos y fuentes en docs/solutions/groovebox/dsp.md, "Mezcla, efectos y master (H9)".

use std::f32::consts::TAU;

use super::dsp::Svf;
use crate::mix::{
    DelaySettings, ReverbSettings, delay_feedback, delay_tone_hz, reverb_damping_hz,
    reverb_predelay_s, reverb_rt60,
};

/// Por debajo de esto (−100 dBFS) un efecto se considera en silencio.
const QUIET: f32 = 1.0e-5;

/// Coeficiente de un one-pole pasa-bajos con corte `hz`.
fn lowpass_coef(hz: f32, sample_rate: f32) -> f32 {
    1.0 - (-TAU * hz / sample_rate).exp()
}

/// Parámetro suavizado por bloque: un one-pole evaluado al final del bloque y una rampa lineal
/// adentro. Cuesta una suma por muestra y, quieto, nada (la ganancia se aplica constante).
#[derive(Clone, Copy, Debug)]
pub struct Ramp {
    pub value: f32,
    pub target: f32,
    /// `exp(-1 / (tau·fs))`: lo que queda de la distancia al objetivo después de una muestra.
    keep: f32,
}

impl Ramp {
    pub fn new(value: f32, tau: f32, sample_rate: f32) -> Self {
        Self {
            value,
            target: value,
            keep: (-1.0 / (tau.max(1.0e-5) * sample_rate)).exp(),
        }
    }

    /// Salta al objetivo (cuando no hay señal que proteger).
    pub fn settle(&mut self) {
        self.value = self.target;
    }

    pub fn is_settled(&self) -> bool {
        self.value == self.target
    }

    pub fn is_zero(&self) -> bool {
        self.value == 0.0 && self.target == 0.0
    }

    /// Avanza `n` muestras: devuelve el valor al principio y el incremento por muestra.
    pub fn advance(&mut self, n: usize) -> (f32, f32) {
        let start = self.value;
        if start == self.target || n == 0 {
            return (start, 0.0);
        }
        let mut end = self.target + (start - self.target) * self.keep.powi(n as i32);
        // −100 dB de una ganancia unitaria: el último escalón no se oye.
        if (end - self.target).abs() < 1.0e-5 {
            end = self.target;
        }
        self.value = end;
        (start, (end - start) / n as f32)
    }
}

/// `dst += src · ramp`, con la rampa avanzada `src.len()` muestras. ~2 operaciones por muestra.
pub fn mix_into(dst: &mut [f32], src: &[f32], ramp: &mut Ramp) {
    if ramp.is_zero() {
        return;
    }
    let (g0, inc) = ramp.advance(src.len());
    if inc == 0.0 {
        for (d, &s) in dst.iter_mut().zip(src) {
            *d += s * g0;
        }
    } else {
        let mut g = g0;
        for (d, &s) in dst.iter_mut().zip(src) {
            g += inc;
            *d += s * g;
        }
    }
}

fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

/// Delay estéreo de envío (spec-motor, "Efectos"): un buffer circular por canal, lectura con
/// interpolación lineal (el tiempo se desliza al cambiar el tempo o el ajuste), pasa-bajos de un
/// polo en la realimentación y cruce L/R para el ping-pong.
pub struct Delay {
    left: Box<[f32]>,
    right: Box<[f32]>,
    write: usize,
    /// Tiempo en muestras (se desliza: cambiarlo suena como una cinta, no hace click).
    time: Ramp,
    feedback: Ramp,
    tone: f32,
    width: f32,
    lp: [f32; 2],
    quiet: usize,
    active: bool,
}

/// 2^18 muestras por canal (5,46 s a 48 kHz; 1 MB cada uno): un compás entra desde 44 BPM.
pub const DELAY_LEN: usize = 1 << 18;
const DELAY_TIME_TAU: f32 = 0.05;
const DELAY_FEEDBACK_TAU: f32 = 0.02;

impl Delay {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            left: vec![0.0; DELAY_LEN].into_boxed_slice(),
            right: vec![0.0; DELAY_LEN].into_boxed_slice(),
            write: 0,
            time: Ramp::new(1.0, DELAY_TIME_TAU, sample_rate),
            feedback: Ramp::new(0.0, DELAY_FEEDBACK_TAU, sample_rate),
            tone: 1.0,
            width: 0.0,
            lp: [0.0; 2],
            quiet: 0,
            active: false,
        }
    }

    /// Sigue sonando (cola) o recibió señal hace menos de un buffer.
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set(&mut self, s: &DelaySettings, bpm: f32, sample_rate: f32) {
        let max = (DELAY_LEN - 4) as f32;
        self.time.target = (s.time.seconds(bpm) * sample_rate).clamp(1.0, max);
        self.feedback.target = delay_feedback(s.feedback);
        if !self.active {
            // En silencio no hay nada que deslizar.
            self.time.settle();
            self.feedback.settle();
        }
        self.tone = lowpass_coef(delay_tone_hz(s.tone), sample_rate);
        self.width = f32::from(s.width.min(127)) / 127.0;
    }

    /// Lee los buses de envío y escribe la salida del delay (sin el nivel de retorno).
    /// ~30 operaciones por frame estéreo.
    pub fn process(&mut self, in_l: &[f32], in_r: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = in_l.len();
        let (mut d, d_inc) = self.time.advance(n);
        let (mut fb, fb_inc) = self.feedback.advance(n);
        let mask = DELAY_LEN - 1;
        let (a, b) = (1.0 - self.width, self.width);
        let tone = self.tone;
        let (l, r) = (&mut self.left, &mut self.right);
        let mut w = self.write;
        let mut lp = self.lp;
        for i in 0..n {
            d += d_inc;
            fb += fb_inc;
            // `w + LEN - d` es positivo: el piso es el índice entero.
            let pos = (w + DELAY_LEN) as f32 - d;
            let i0 = pos as usize;
            let frac = pos - i0 as f32;
            let (i0, i1) = (i0 & mask, (i0 + 1) & mask);
            let yl = l[i0] + (l[i1] - l[i0]) * frac;
            let yr = r[i0] + (r[i1] - r[i0]) * frac;
            lp[0] += (yl - lp[0]) * tone;
            lp[1] += (yr - lp[1]) * tone;
            let (xl, xr) = (in_l[i], in_r[i]);
            // Con ancho 1 entra todo por la izquierda y la realimentación se cruza: ping-pong.
            l[w] = a * xl + b * 0.5 * (xl + xr) + fb * (a * lp[0] + b * lp[1]);
            r[w] = a * xr + fb * (a * lp[1] + b * lp[0]);
            w = (w + 1) & mask;
            out_l[i] = yl;
            out_r[i] = yr;
        }
        self.write = w;
        self.lp = lp;
        let level = peak(in_l)
            .max(peak(in_r))
            .max(peak(&out_l[..n]))
            .max(peak(&out_r[..n]));
        self.track_silence(level, n);
    }

    /// Se apaga después de un buffer entero en silencio: para entonces todo lo que queda en las
    /// líneas está por debajo de −100 dB (y no reaparece si después se alarga el tiempo).
    fn track_silence(&mut self, level: f32, n: usize) {
        if level >= QUIET {
            self.quiet = 0;
            self.active = true;
        } else {
            self.quiet += n;
            if self.quiet > DELAY_LEN {
                self.active = false;
                self.time.settle();
                self.feedback.settle();
            }
        }
    }
}

/// Línea de retardo de largo fijo (sin interpolar): una lectura y una escritura por muestra.
struct Line {
    buf: Box<[f32]>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)].into_boxed_slice(),
            pos: 0,
        }
    }

    #[inline]
    fn read(&self) -> f32 {
        self.buf[self.pos]
    }

    #[inline]
    fn write_advance(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
    }

    /// Allpass de Schroeder: `v = x + g·v[n−M]`, `y = v[n−M] − g·v`. ~4 operaciones.
    #[inline]
    fn allpass(&mut self, x: f32, g: f32) -> f32 {
        let delayed = self.read();
        let v = x + g * delayed;
        self.write_advance(v);
        delayed - g * v
    }
}

/// Largos de las 4 líneas del FDN a 48 kHz (32–55 ms, primos entre sí) y de los 2 allpass de
/// difusión de la entrada (5 y 8 ms). Se escalan con la frecuencia de muestreo.
const FDN_LINES: [usize; 4] = [1553, 1871, 2243, 2647];
const DIFFUSERS: [usize; 2] = [241, 383];
const DIFFUSION: f32 = 0.6;
/// Predelay de hasta 100 ms: 8192 muestras alcanzan hasta 81 kHz.
const PREDELAY_LEN: usize = 8192;
const PREDELAY_TAU: f32 = 0.05;
/// Nivel de salida: con el retorno en 100, una cola de ~1 s queda unos 6 dB por debajo de la
/// señal seca (medido con el test `reverb_tail_follows_rt60`).
const REVERB_OUT: f32 = 0.3;
/// Fracción de la distancia que recorren las ganancias de las líneas por bloque al cambiar el
/// tamaño (sin escalones bruscos en la cola).
const GAIN_GLIDE: f32 = 0.2;

/// Reverb liviana: predelay → 2 allpass de difusión → FDN de 4 líneas con matriz de Hadamard y
/// un pasa-bajos de amortiguación por línea (Jot; Smith, *Physical Audio Signal Processing*).
/// Entrada mono, salida estéreo (pares de líneas distintos en cada canal).
pub struct Reverb {
    pre: Box<[f32]>,
    pre_write: usize,
    predelay: Ramp,
    diffusers: [Line; 2],
    lines: [Line; 4],
    gains: [f32; 4],
    gain_targets: [f32; 4],
    damp: f32,
    lp: [f32; 4],
    quiet: usize,
    /// Muestras de silencio para darla por terminada (todo el camino de la señal).
    flush_len: usize,
    active: bool,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 48_000.0;
        let len = |l: usize| ((l as f32 * scale).round() as usize).max(1);
        let lines = FDN_LINES.map(|l| Line::new(len(l)));
        let diffusers = DIFFUSERS.map(|l| Line::new(len(l)));
        let flush_len = PREDELAY_LEN
            + diffusers.iter().map(|d| d.buf.len()).sum::<usize>()
            + lines.iter().map(|l| l.buf.len()).max().unwrap_or(0);
        Self {
            pre: vec![0.0; PREDELAY_LEN].into_boxed_slice(),
            pre_write: 0,
            predelay: Ramp::new(0.0, PREDELAY_TAU, sample_rate),
            diffusers,
            lines,
            gains: [0.0; 4],
            gain_targets: [0.0; 4],
            damp: 1.0,
            lp: [0.0; 4],
            quiet: 0,
            flush_len,
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set(&mut self, s: &ReverbSettings, sample_rate: f32) {
        let rt60 = reverb_rt60(s.size);
        for (g, line) in self.gain_targets.iter_mut().zip(&self.lines) {
            // −60 dB en `rt60` contando las vueltas por esta línea.
            *g = 10f32.powf(-3.0 * line.buf.len() as f32 / (rt60 * sample_rate));
        }
        self.damp = lowpass_coef(reverb_damping_hz(s.damping), sample_rate);
        self.predelay.target =
            (reverb_predelay_s(s.predelay) * sample_rate).clamp(1.0, (PREDELAY_LEN - 2) as f32);
        if !self.active {
            self.gains = self.gain_targets;
            self.predelay.settle();
        }
    }

    /// Lee el bus de envío (mono) y escribe la salida estéreo (sin el nivel de retorno).
    /// ~55 operaciones por frame.
    pub fn process(&mut self, input: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = input.len();
        for (g, t) in self.gains.iter_mut().zip(self.gain_targets) {
            *g += (t - *g) * GAIN_GLIDE;
        }
        let g = self.gains;
        let damp = self.damp;
        let (mut d, d_inc) = self.predelay.advance(n);
        let mask = PREDELAY_LEN - 1;
        let mut lp = self.lp;
        for i in 0..n {
            d += d_inc;
            self.pre[self.pre_write] = input[i];
            let pos = (self.pre_write + PREDELAY_LEN) as f32 - d;
            let i0 = pos as usize;
            let frac = pos - i0 as f32;
            let (i0, i1) = (i0 & mask, (i0 + 1) & mask);
            let mut x = self.pre[i0] + (self.pre[i1] - self.pre[i0]) * frac;
            self.pre_write = (self.pre_write + 1) & mask;

            for ap in &mut self.diffusers {
                x = ap.allpass(x, DIFFUSION);
            }
            let y = [
                self.lines[0].read(),
                self.lines[1].read(),
                self.lines[2].read(),
                self.lines[3].read(),
            ];
            let mut s = [0.0f32; 4];
            for k in 0..4 {
                lp[k] += (y[k] - lp[k]) * damp;
                s[k] = lp[k] * g[k];
            }
            // Hadamard 4×4 normalizada (ortogonal): mezcla sin ganar energía.
            let (a, b) = (s[0] + s[1], s[0] - s[1]);
            let (c, e) = (s[2] + s[3], s[2] - s[3]);
            let h = [0.5 * (a + c), 0.5 * (b + e), 0.5 * (a - c), 0.5 * (b - e)];
            self.lines[0].write_advance(h[0] + x);
            self.lines[1].write_advance(h[1] - x);
            self.lines[2].write_advance(h[2] + x);
            self.lines[3].write_advance(h[3] - x);
            out_l[i] = REVERB_OUT * (y[0] + y[2]);
            out_r[i] = REVERB_OUT * (y[1] + y[3]);
        }
        self.lp = lp;
        let level = peak(input).max(peak(&out_l[..n])).max(peak(&out_r[..n]));
        if level >= QUIET {
            self.quiet = 0;
            self.active = true;
        } else {
            self.quiet += n;
            if self.quiet > self.flush_len {
                self.active = false;
                self.gains = self.gain_targets;
                self.predelay.settle();
            }
        }
    }
}

/// Umbral del limitador: 0 dBFS a la entrada del soft clip (que después lo lleva a −2,2 dBFS).
pub const LIMIT: f32 = 1.0;
const LIMITER_HOLD: f32 = 0.025;
const LIMITER_RELEASE_TAU: f32 = 0.12;

/// Limitador de picos sin anticipación: envolvente de pico con ataque instantáneo, retención de
/// 25 ms (más que un período de 40 Hz: el bombo no se distorsiona por la relajación) y
/// relajación exponencial de 120 ms; ganancia `LIMIT / envolvente` cuando la pasa. Ningún pico
/// supera `LIMIT`. Por debajo del umbral no toca la señal (bit a bit).
pub struct Limiter {
    env: f32,
    hold: u32,
    hold_len: u32,
    release: f32,
    /// Menor ganancia aplicada desde la última lectura (para el medidor).
    pub min_gain: f32,
}

impl Limiter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            env: 0.0,
            hold: 0,
            hold_len: (LIMITER_HOLD * sample_rate) as u32,
            release: (-1.0 / (LIMITER_RELEASE_TAU * sample_rate)).exp(),
            min_gain: 1.0,
        }
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
        self.hold = 0;
    }

    /// ~8 operaciones por frame; una división por muestra solo mientras limita.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (x, y) in l.iter_mut().zip(r.iter_mut()) {
            let p = x.abs().max(y.abs());
            let mut decayed = if self.hold > 0 {
                self.hold -= 1;
                self.env
            } else {
                self.env * self.release
            };
            if decayed < LIMIT {
                // Ya no limita: se olvida (y nunca baja a subnormales).
                decayed = 0.0;
            }
            if p >= decayed {
                self.env = p;
                self.hold = self.hold_len;
            } else {
                self.env = decayed;
            }
            if self.env > LIMIT {
                let g = LIMIT / self.env;
                *x *= g;
                *y *= g;
                self.min_gain = self.min_gain.min(g);
            }
        }
    }
}

/// Filtro de bus del stick izquierdo en las pistas sin filtro propio (percusión, sample): a la
/// izquierda un pasa-bajos que cierra de 20 kHz a 110 Hz, a la derecha un pasa-altos que abre de
/// 20 Hz a 5 kHz (filtro de DJ). Cerca del centro se mezcla con la señal seca: entra y sale sin
/// clicks. ~14 operaciones por muestra mientras actúa; en el centro no cuesta nada.
#[derive(Clone, Copy, Debug, Default)]
pub struct BusFilter {
    svf: Svf,
    wet: f32,
}

impl BusFilter {
    /// `cutoff` y `resonance` son los desplazamientos de los sticks, en −1..=1.
    pub fn process(&mut self, buf: &mut [f32], cutoff: f32, resonance: f32, sample_rate: f32) {
        let target = (cutoff.abs() * 10.0).min(1.0);
        if target == 0.0 && self.wet == 0.0 {
            return;
        }
        if self.wet == 0.0 {
            self.svf = Svf::default();
        }
        let lowpass = cutoff < 0.0;
        let fc = if lowpass {
            20_000.0 * (7.5 * cutoff).exp2()
        } else {
            20.0 * (8.0 * cutoff).exp2()
        };
        let k = std::f32::consts::SQRT_2 - 1.1 * resonance.max(0.0);
        self.svf.set(fc, k, sample_rate);
        let mut wet = self.wet;
        let inc = (target - wet) / buf.len().max(1) as f32;
        for s in buf.iter_mut() {
            let o = self.svf.process(*s);
            let f = if lowpass { o.low } else { o.high };
            wet += inc;
            *s += (f - *s) * wet;
        }
        self.wet = target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mix::{DelayTime, FxSettings};

    const SR: f32 = 48_000.0;

    #[test]
    fn ramp_reaches_its_target_without_steps() {
        let mut r = Ramp::new(0.0, 0.01, SR);
        r.target = 1.0;
        let mut prev = 0.0;
        let mut out = [0.0f32; 32];
        for _ in 0..200 {
            out.fill(0.0);
            mix_into(&mut out, &[1.0; 32], &mut r);
            for &g in &out {
                assert!(g >= prev - 1e-6 && g - prev < 0.01, "salto {prev} → {g}");
                prev = g;
            }
        }
        assert_eq!(r.value, 1.0);
        assert!(r.is_settled());
    }

    fn impulse_delay(width: u8) -> (Vec<f32>, Vec<f32>, usize) {
        let mut d = Delay::new(SR);
        let s = DelaySettings {
            time: DelayTime::Sixteenth,
            feedback: 64,
            tone: 127,
            width,
            return_level: 100,
        };
        d.set(&s, 120.0, SR);
        let step = (DelayTime::Sixteenth.seconds(120.0) * SR) as usize;
        let n = step * 4;
        let mut in_l = vec![0.0f32; n];
        let mut in_r = vec![0.0f32; n];
        in_l[0] = 1.0;
        in_r[0] = 1.0;
        let (mut out_l, mut out_r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for start in (0..n).step_by(32) {
            let end = (start + 32).min(n);
            d.process(
                &in_l[start..end],
                &in_r[start..end],
                &mut out_l[start..end],
                &mut out_r[start..end],
            );
        }
        (out_l, out_r, step)
    }

    #[test]
    fn delay_echoes_at_the_tempo_time_and_ping_pongs() {
        // Estéreo (ancho 0): el eco llega a los dos canales justo a 1/16 de 120 BPM (125 ms).
        let (l, r, step) = impulse_delay(0);
        let first = l.iter().position(|x| x.abs() > 0.1).unwrap();
        assert_eq!(first, step);
        assert!((l[step] - 1.0).abs() < 1e-6 && (r[step] - 1.0).abs() < 1e-6);
        // El segundo eco, atenuado por la realimentación (64/127 · 0,95 ≈ 0,48).
        let second = l[2 * step - 2..2 * step + 3]
            .iter()
            .fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(second > 0.3 && second < 0.6, "segundo eco {second}");
        // Ping-pong (ancho 127): primero a la izquierda, después a la derecha.
        let (l, r, step) = impulse_delay(127);
        assert!(l[step] > 0.9 && r[step].abs() < 1e-6);
        let r2 = r[2 * step - 2..2 * step + 3]
            .iter()
            .fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(r2 > 0.3 && l[2 * step].abs() < 0.05, "derecha {r2}");
    }

    #[test]
    fn delay_goes_idle_after_a_silent_buffer() {
        let mut d = Delay::new(SR);
        d.set(&FxSettings::default().delay, 120.0, SR);
        let mut in_l = [0.0f32; 32];
        in_l[0] = 0.5;
        let (mut ol, mut or) = ([0.0f32; 32], [0.0f32; 32]);
        d.process(&in_l, &[0.0; 32], &mut ol, &mut or);
        assert!(d.is_active());
        let mut blocks = 0;
        while d.is_active() {
            d.process(&[0.0; 32], &[0.0; 32], &mut ol, &mut or);
            blocks += 1;
            assert!(blocks < 100_000);
        }
        // Al menos un buffer entero (5,5 s) después de que los ecos bajaron de −100 dB.
        assert!(blocks * 32 > DELAY_LEN, "{blocks} bloques");
    }

    /// RT60 medido con la energía de la cola (integración de Schroeder simplificada: nivel RMS
    /// en ventanas de 50 ms) contra el ajuste.
    #[test]
    fn reverb_tail_follows_rt60() {
        let mut rv = Reverb::new(SR);
        let s = ReverbSettings {
            size: 60,
            damping: 0,
            predelay: 0,
            return_level: 100,
        };
        rv.set(&s, SR);
        let n = (SR * 3.0) as usize;
        let mut input = vec![0.0f32; n];
        input[0] = 1.0;
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for start in (0..n).step_by(32) {
            let end = (start + 32).min(n);
            rv.process(&input[start..end], &mut l[start..end], &mut r[start..end]);
        }
        assert!(l.iter().chain(&r).all(|x| x.is_finite()));
        let win = (SR * 0.05) as usize;
        let rms_db = |from: usize| {
            let e: f32 = l[from..from + win].iter().map(|x| x * x).sum::<f32>()
                + r[from..from + win].iter().map(|x| x * x).sum::<f32>();
            10.0 * (e / (2 * win) as f32).log10()
        };
        let t1 = (SR * 0.2) as usize;
        let t2 = (SR * 0.7) as usize;
        let slope = (rms_db(t2) - rms_db(t1)) / 0.5; // dB por segundo
        let measured = -60.0 / slope;
        let expected = reverb_rt60(60);
        assert!(
            (measured / expected - 1.0).abs() < 0.25,
            "RT60 medido {measured:.2} s, esperado {expected:.2} s"
        );
        // Los canales están decorrelacionados (no es mono).
        let dot: f32 = l.iter().zip(&r).map(|(a, b)| a * b).sum();
        let el: f32 = l.iter().map(|a| a * a).sum();
        let er: f32 = r.iter().map(|b| b * b).sum();
        assert!((dot / (el * er).sqrt()).abs() < 0.5);
    }

    #[test]
    fn limiter_never_exceeds_the_threshold_and_is_transparent_below() {
        let mut lim = Limiter::new(SR);
        // Debajo del umbral: bit a bit.
        let mut l: Vec<f32> = (0..4800).map(|i| 0.9 * (i as f32 * 0.05).sin()).collect();
        let mut r = l.clone();
        let orig = l.clone();
        lim.process(&mut l, &mut r);
        assert_eq!(l, orig);
        assert_eq!(lim.min_gain, 1.0);
        // Un seno de 50 Hz a +6 dB: nunca pasa de 1 y la ganancia no tiembla dentro del ciclo.
        let mut l: Vec<f32> = (0..48_000)
            .map(|i| 2.0 * (TAU * 50.0 * i as f32 / SR).sin())
            .collect();
        let mut r = l.clone();
        lim.process(&mut l, &mut r);
        assert!(l.iter().all(|x| x.abs() <= LIMIT + 1e-6));
        let tail_peak = l[24_000..].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((tail_peak - 1.0).abs() < 1e-3, "pico {tail_peak}");
        assert!((lim.min_gain - 0.5).abs() < 1e-3);
        // Sin la retención, la relajación aplastaría cada medio ciclo: con ella el seno sigue
        // siendo un seno (el segundo armónico, casi nada).
        let (mut c2, mut s2) = (0.0f32, 0.0f32);
        let mut c1 = 0.0f32;
        for (i, &x) in l[24_000..].iter().enumerate() {
            let t = TAU * 50.0 * i as f32 / SR;
            c1 += x * t.sin();
            c2 += x * (2.0 * t).cos();
            s2 += x * (2.0 * t).sin();
        }
        assert!((c2 * c2 + s2 * s2).sqrt() < 0.01 * c1.abs());
    }

    #[test]
    fn bus_filter_fades_in_and_out_without_jumps() {
        let mut f = BusFilter::default();
        let mut prev = 0.0f32;
        let mut max_jump = 0.0f32;
        let mut phase = 0.0f32;
        // El stick va al extremo izquierdo y vuelve, en bloques de 32 con el suavizado de la
        // performance; la señal es un seno de 100 Hz.
        let mut x = 0.0f32;
        for block in 0..600 {
            let target = if block < 300 { -1.0 } else { 0.0 };
            x += (target - x) * 0.05;
            if block == 599 {
                x = 0.0;
            }
            let mut buf = [0.0f32; 32];
            for s in &mut buf {
                *s = 0.5 * (TAU * phase).sin();
                phase = (phase + 100.0 / SR).fract();
            }
            f.process(&mut buf, x, 0.0, SR);
            for &s in &buf {
                max_jump = max_jump.max((s - prev).abs());
                prev = s;
            }
        }
        // El salto entre muestras de un seno de 100 Hz con amplitud 0,5 es 0,0065; al abrir el
        // filtro rápido (de 110 a 290 Hz en 3 bloques) la amplitud crece y llega a 0,010. Un
        // click (entrar al filtro con el estado en cero) daría más de 0,1.
        assert!(max_jump < 0.013, "salto {max_jump}");
    }
}
