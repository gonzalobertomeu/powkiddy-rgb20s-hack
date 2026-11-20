//! Primitivas de DSP: ruido, seno polinómico, PolyBLEP, SVF TPT, soft clip, suavizado,
//! coeficientes de envolvente y flush-to-zero. Fórmulas y fuentes en
//! docs/solutions/groovebox/dsp.md.

use std::f32::consts::{PI, TAU};

/// −80 dB: por debajo, una envolvente se da por terminada (y la voz se libera).
pub const SILENCE: f32 = 1.0e-4;

/// Coeficiente multiplicativo para caer 60 dB en `seconds`.
pub fn decay_coef(seconds: f32, sample_rate: f32) -> f32 {
    (0.001f32.ln() / (seconds.max(1.0e-4) * sample_rate)).exp()
}

/// Coeficiente de un one-pole `y += (x - y) * c` con constante de tiempo `tau`.
pub fn one_pole_coef(tau: f32, sample_rate: f32) -> f32 {
    1.0 - (-1.0 / (tau.max(1.0e-5) * sample_rate)).exp()
}

/// Xorshift32 (Marsaglia): ruido blanco determinístico, sin memoria dinámica.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        // El estado 0 es un punto fijo de xorshift.
        Self(seed.wrapping_mul(0x9E37_79B9) | 1)
    }

    /// Uniforme en [-1, 1).
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as i32) as f32 * (1.0 / 2_147_483_648.0)
    }
}

/// `sin(2π·p)` para `p` en [0, 1). Reflexión a [-1/4, 1/4] y Taylor impar hasta x⁹:
/// error < 4e-6, ~12 operaciones, sin llamar a libm.
#[inline]
pub fn sin_turns(p: f32) -> f32 {
    let q = if p < 0.25 {
        p
    } else if p < 0.75 {
        0.5 - p
    } else {
        p - 1.0
    };
    let x = q * TAU;
    let x2 = x * x;
    x * (1.0
        + x2 * (-1.0 / 6.0 + x2 * (1.0 / 120.0 + x2 * (-1.0 / 5040.0 + x2 * (1.0 / 362_880.0)))))
}

/// Residuo PolyBLEP de 2 muestras (Välimäki y Huovilainen) para una discontinuidad de +2 en `t = 0`.
#[inline]
pub fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

/// Soft clip: aproximante de Padé de `tanh`, `x(27 + x²)/(27 + 9x²)`, que vale ±1 con derivada
/// 0 en ±3; afuera se satura. Continua y con derivada continua: no agrega clicks.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// Filtro de estado variable con integradores trapezoidales (TPT; Zavalishin, cap. 4; Simper).
/// Estable con modulación rápida. Salidas simultáneas pasa-bajos, pasa-banda y pasa-altos.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SvfOut {
    pub low: f32,
    pub band: f32,
    pub high: f32,
}

impl Svf {
    /// `k = 1/Q`: 2 sin resonancia, cerca de 0 autooscila. Llama a `tan`: una vez por nota o por
    /// subbloque, no por muestra.
    pub fn set(&mut self, cutoff: f32, k: f32, sample_rate: f32) {
        let fc = cutoff.clamp(10.0, 0.45 * sample_rate);
        let g = (PI * fc / sample_rate).tan();
        self.k = k;
        self.a1 = 1.0 / (1.0 + g * (g + k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> SvfOut {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut {
            low: v2,
            band: v1,
            high: x - self.k * v1 - v2,
        }
    }
}

/// One-pole hacia un objetivo, para parámetros que cambian mientras suena (sin zipper noise).
#[derive(Clone, Copy, Debug)]
pub struct Smoother {
    pub value: f32,
    pub target: f32,
    coef: f32,
}

impl Smoother {
    pub fn new(value: f32, tau: f32, sample_rate: f32) -> Self {
        Self {
            value,
            target: value,
            coef: one_pole_coef(tau, sample_rate),
        }
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        self.value += (self.target - self.value) * self.coef;
        self.value
    }

    /// Salta al objetivo (cuando no hay señal que proteger) y evita colas subnormales.
    pub fn settle(&mut self) {
        self.value = self.target;
    }

    pub fn is_settled(&self) -> bool {
        (self.target - self.value).abs() < 1.0e-6
    }
}

/// Activa flush-to-zero en el hilo actual: los subnormales se tratan como 0 y no disparan
/// el camino lento del FPU. aarch64: bit FZ (24) del FPCR. x86_64: FTZ (15) y DAZ (6) del MXCSR.
/// Es por hilo: hay que llamarla en el hilo de audio (o en el de un render offline).
pub fn enable_flush_to_zero() {
    #[cfg(target_arch = "aarch64")]
    // SAFETY: solo cambia el modo de redondeo de subnormales del hilo actual; Rust no depende
    // de subnormales en este código (ver dsp.md, "Denormales").
    unsafe {
        let mut fpcr: u64;
        std::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack, preserves_flags));
        fpcr |= 1 << 24;
        std::arch::asm!("msr fpcr, {}", in(reg) fpcr, options(nomem, nostack, preserves_flags));
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: ídem, con el registro MXCSR de SSE.
    unsafe {
        let mut csr: u32 = 0;
        std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
        csr |= (1 << 15) | (1 << 6);
        std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, preserves_flags, readonly));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    #[test]
    fn sin_turns_is_accurate() {
        let mut max_err = 0.0f32;
        for i in 0..10_000 {
            let p = i as f32 / 10_000.0;
            max_err = max_err.max((sin_turns(p) - (p * TAU).sin()).abs());
        }
        assert!(max_err < 1.0e-5, "error {max_err}");
    }

    #[test]
    fn rng_is_bounded_and_centered() {
        let mut rng = Rng::new(7);
        let mut sum = 0.0f64;
        for _ in 0..100_000 {
            let x = rng.bipolar();
            assert!((-1.0..1.0).contains(&x));
            sum += f64::from(x);
        }
        assert!((sum / 100_000.0).abs() < 0.01);
    }

    #[test]
    fn soft_clip_is_bounded_monotonic_and_smooth() {
        let mut prev = soft_clip(-10.0);
        assert_eq!(prev, -1.0);
        for i in -1000..=1000 {
            let y = soft_clip(i as f32 / 100.0);
            // Cerca de ±3 la derivada es 0: se tolera 1 ulp de redondeo.
            assert!(y >= prev - 1e-6 && y.abs() <= 1.0 + 1e-6, "x {i}: {y}");
            prev = y;
        }
        assert!((soft_clip(0.1) - 0.1f32.tanh()).abs() < 1e-4);
    }

    #[test]
    fn svf_lowpass_passes_dc_and_highpass_blocks_it() {
        let mut f = Svf::default();
        f.set(1000.0, 1.0, 48_000.0);
        let mut out = f.process(1.0);
        for _ in 0..48_000 {
            out = f.process(1.0);
        }
        assert!((out.low - 1.0).abs() < 1e-4);
        assert!(out.high.abs() < 1e-4);
    }

    #[test]
    fn flush_to_zero_flushes_subnormals() {
        enable_flush_to_zero();
        let tiny = black_box(f32::MIN_POSITIVE) * black_box(0.25);
        assert_eq!(tiny, 0.0);
    }
}
