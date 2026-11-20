//! Medidor de carga del hilo de audio: tiempo de render contra la duración del buffer.
//! Lo alimenta el callback (que es el que mide el tiempo); acá solo hay aritmética.

use std::time::Duration;

/// Cada cuánto se publica un reporte (en tiempo de audio).
pub const REPORT_SECONDS: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoadReport {
    /// Tiempo de render / tiempo de audio, en % de un núcleo, promedio de la ventana.
    pub cpu_avg: f32,
    /// El peor callback de la ventana, en % de su propio presupuesto.
    pub cpu_peak: f32,
    /// Máximo de voces sonando en la ventana (medido en cada subbloque, no al final del callback).
    pub voices_max: u8,
    /// Notas disparadas en la ventana (audición y secuenciador).
    pub notes: u32,
    /// Frames del último callback (el período real del backend).
    pub frames: u32,
}

pub struct LoadMeter {
    sample_rate: f32,
    window_frames: u64,
    busy_ns: u64,
    frames: u64,
    peak: f32,
    voices_max: u8,
    notes: u32,
}

impl LoadMeter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            window_frames: (REPORT_SECONDS * sample_rate) as u64,
            busy_ns: 0,
            frames: 0,
            peak: 0.0,
            voices_max: 0,
            notes: 0,
        }
    }

    /// Registra un callback (`voices`: máximo durante el callback; `notes`: disparadas en él);
    /// cada `REPORT_SECONDS` devuelve un reporte y reinicia la ventana.
    pub fn record(
        &mut self,
        busy: Duration,
        frames: usize,
        voices: usize,
        notes: u32,
    ) -> Option<LoadReport> {
        if frames == 0 {
            return None;
        }
        let busy_ns = busy.as_nanos().min(u128::from(u64::MAX)) as u64;
        let budget_ns = frames as f64 * 1.0e9 / f64::from(self.sample_rate);
        self.peak = self.peak.max((busy_ns as f64 / budget_ns * 100.0) as f32);
        self.busy_ns += busy_ns;
        self.frames += frames as u64;
        self.voices_max = self.voices_max.max(voices.min(255) as u8);
        self.notes = self.notes.saturating_add(notes);
        if self.frames < self.window_frames {
            return None;
        }
        let audio_ns = self.frames as f64 * 1.0e9 / f64::from(self.sample_rate);
        let report = LoadReport {
            cpu_avg: (self.busy_ns as f64 / audio_ns * 100.0) as f32,
            cpu_peak: self.peak,
            voices_max: self.voices_max,
            notes: self.notes,
            frames: frames as u32,
        };
        self.busy_ns = 0;
        self.frames = 0;
        self.peak = 0.0;
        self.voices_max = 0;
        self.notes = 0;
        Some(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_average_and_peak_every_window() {
        let mut m = LoadMeter::new(48_000.0);
        // 256 frames = 5,333 ms; 0,5 s = 93,75 callbacks.
        let mut report = None;
        for i in 0..94 {
            let busy = if i == 10 { 2_666_667 } else { 533_333 }; // 50 % y 10 %
            report = report.or(m.record(Duration::from_nanos(busy), 256, i % 5, 1));
        }
        let r = report.expect("un reporte por ventana");
        assert!((r.cpu_peak - 50.0).abs() < 0.1, "{r:?}");
        assert!((r.cpu_avg - 10.43).abs() < 0.1, "{r:?}");
        assert_eq!(r.voices_max, 4);
        assert_eq!(r.notes, 94);
        assert_eq!(r.frames, 256);
    }
}
