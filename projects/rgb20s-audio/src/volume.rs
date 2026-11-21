/// Escala del control de volumen: valores crudos y su rango en dB.
///
/// Supone una escala lineal en dB (TLV `DB_SCALE`/`DB_LINEAR`), como la del RK817:
/// 0..255 = -95..0 dB, unos 0,37 dB por paso crudo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolumeScale {
    pub raw_min: i64,
    pub raw_max: i64,
    pub db_min: f64,
    pub db_max: f64,
}

/// Fracción del rango crudo que mueve un paso si el control no informa dB.
const FALLBACK_STEPS: i64 = 32;

impl VolumeScale {
    fn has_db(&self) -> bool {
        self.db_max > self.db_min && self.raw_max > self.raw_min
    }

    pub fn db_of(&self, raw: i64) -> f64 {
        if !self.has_db() {
            return self.db_max;
        }
        let t = (raw - self.raw_min) as f64 / (self.raw_max - self.raw_min) as f64;
        self.db_min + t * (self.db_max - self.db_min)
    }

    pub fn raw_of(&self, db: f64) -> i64 {
        let t = (db - self.db_min) / (self.db_max - self.db_min);
        let raw = self.raw_min as f64 + t * (self.raw_max - self.raw_min) as f64;
        (raw.round() as i64).clamp(self.raw_min, self.raw_max)
    }

    /// Porcentaje para las apps: lineal en dB (`audio_state::volume_percent`); si el control no
    /// informa dB, la fracción del rango crudo.
    pub fn percent(&self, raw: i64) -> u8 {
        if self.has_db() {
            return rgb20s_common::audio_state::volume_percent(self.db_of(raw), self.db_max);
        }
        let span = (self.raw_max - self.raw_min).max(1) as f64;
        let t = (raw.clamp(self.raw_min, self.raw_max) - self.raw_min) as f64 / span;
        (t * 100.0).round() as u8
    }

    /// Valor crudo después de subir (`up`) o bajar un paso de `step_db`.
    /// Siempre se mueve al menos un valor crudo, salvo en los extremos.
    pub fn step(&self, raw: i64, up: bool, step_db: f64) -> i64 {
        let raw = raw.clamp(self.raw_min, self.raw_max);
        let sign = if up { 1 } else { -1 };
        let mut next = if self.has_db() {
            self.raw_of(self.db_of(raw) + sign as f64 * step_db)
        } else {
            raw + sign * ((self.raw_max - self.raw_min) / FALLBACK_STEPS).max(1)
        };
        if next == raw {
            next = raw + sign;
        }
        next.clamp(self.raw_min, self.raw_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RK817: VolumeScale = VolumeScale {
        raw_min: 0,
        raw_max: 255,
        db_min: -95.0,
        db_max: 0.0,
    };

    #[test]
    fn db_conversion_matches_rk817() {
        assert_eq!(RK817.db_of(255), 0.0);
        assert_eq!(RK817.db_of(0), -95.0);
        // 201 crudo ≈ -20,1 dB (valor medido en la consola)
        assert!((RK817.db_of(201) - -20.12).abs() < 0.01);
        assert_eq!(RK817.raw_of(-20.12), 201);
    }

    #[test]
    fn three_db_step_is_about_eight_raw() {
        assert_eq!(RK817.step(201, true, 3.0), 209);
        assert_eq!(RK817.step(201, false, 3.0), 193);
        let db = RK817.db_of(RK817.step(100, true, 3.0)) - RK817.db_of(100);
        assert!((db - 3.0).abs() < 0.2, "{db}");
    }

    #[test]
    fn clamps_at_the_ends() {
        assert_eq!(RK817.step(254, true, 3.0), 255);
        assert_eq!(RK817.step(255, true, 3.0), 255);
        assert_eq!(RK817.step(2, false, 3.0), 0);
        assert_eq!(RK817.step(0, false, 3.0), 0);
        assert_eq!(RK817.step(300, false, 3.0), 247);
    }

    #[test]
    fn tiny_step_still_moves() {
        assert_eq!(RK817.step(100, true, 0.1), 101);
        assert_eq!(RK817.step(100, false, 0.1), 99);
    }

    #[test]
    fn up_then_down_returns() {
        for raw in [0, 37, 128, 201, 250] {
            let up = RK817.step(raw, true, 3.0);
            if up < RK817.raw_max {
                assert_eq!(RK817.step(up, false, 3.0), raw);
            }
        }
    }

    #[test]
    fn without_db_info_uses_fraction_of_range() {
        let scale = VolumeScale {
            raw_min: 0,
            raw_max: 64,
            db_min: 0.0,
            db_max: 0.0,
        };
        assert_eq!(scale.step(10, true, 3.0), 12);
        assert_eq!(scale.step(1, false, 3.0), 0);
        assert_eq!(scale.percent(32), 50);
    }

    #[test]
    fn percent_follows_db() {
        assert_eq!(RK817.percent(255), 100);
        assert_eq!(RK817.percent(201), 66);
        assert_eq!(RK817.percent(0), 0);
        // Una pulsación de 3 dB mueve unos 5 %.
        let up = RK817.percent(RK817.step(201, true, 3.0));
        assert!((70..=72).contains(&up), "{up}");
    }
}
