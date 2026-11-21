//! Pantalla Mezcla (H9, spec-ui "6. Mezcla"): cursor, edición de los canales, los efectos y el
//! master, y los medidores que manda el motor. Sin SDL: el dibujo está en `ui/mix.rs`.
//!
//! El canal del cursor es la pista actual (`App::track`, la misma de las otras pantallas);
//! `MixCursor::master` marca la columna MASTER (solo en la fila del volumen).

use crate::app::App;
use crate::engine::Meters;
use crate::gestures::{Dir, Gesture};
use crate::mix::{
    DelayTime, FxSettings, MASTER_GAIN_MAX, MASTER_GAIN_MIN, MasterSettings, delay_tone_hz,
    level_db, pan_label, reverb_damping_hz, reverb_predelay_s, reverb_rt60,
};
use crate::model::{ChannelStrip, TRACKS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MixRow {
    Volume,
    Pan,
    DelaySend,
    ReverbSend,
    Mute,
    Solo,
    Delay,
    Reverb,
    Master,
}

impl MixRow {
    pub const ALL: [MixRow; 9] = [
        MixRow::Volume,
        MixRow::Pan,
        MixRow::DelaySend,
        MixRow::ReverbSend,
        MixRow::Mute,
        MixRow::Solo,
        MixRow::Delay,
        MixRow::Reverb,
        MixRow::Master,
    ];

    /// Filas de canal (una columna por pista); las otras son de efectos.
    pub fn is_channel(self) -> bool {
        !matches!(self, MixRow::Delay | MixRow::Reverb | MixRow::Master)
    }

    /// Nombre de la fila como en el mockup.
    pub fn label(self) -> &'static str {
        match self {
            MixRow::Volume => "volumen",
            MixRow::Pan => "paneo",
            MixRow::DelaySend => "delay",
            MixRow::ReverbSend => "reverb",
            MixRow::Mute => "mute",
            MixRow::Solo => "solo",
            MixRow::Delay => "DELAY",
            MixRow::Reverb => "REVERB",
            MixRow::Master => "MASTER",
        }
    }

    /// Parámetros de una fila de efectos (nombre corto y largo).
    pub fn items(self) -> &'static [(&'static str, &'static str)] {
        match self {
            MixRow::Delay => &[
                ("tiempo", "TIEMPO DEL DELAY"),
                ("realim.", "REALIMENTACIÓN DEL DELAY"),
                ("tono", "TONO DEL DELAY"),
                ("ancho", "ANCHO DEL DELAY"),
                ("retorno", "RETORNO DEL DELAY"),
            ],
            MixRow::Reverb => &[
                ("tamaño", "TAMAÑO DE LA REVERB"),
                ("amort.", "AMORTIGUACIÓN DE LA REVERB"),
                ("predelay", "PREDELAY DE LA REVERB"),
                ("retorno", "RETORNO DE LA REVERB"),
            ],
            MixRow::Master => &[("limitador", "LIMITADOR"), ("ganancia", "GANANCIA MASTER")],
            _ => &[],
        }
    }

    fn is_toggle(self) -> bool {
        matches!(self, MixRow::Mute | MixRow::Solo)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixCursor {
    pub row: usize,
    /// Parámetro dentro de una fila de efectos.
    pub item: usize,
    /// Columna MASTER de la fila del volumen (la ganancia master).
    pub master: bool,
}

impl MixCursor {
    pub fn row(&self) -> MixRow {
        MixRow::ALL[self.row.min(MixRow::ALL.len() - 1)]
    }
}

/// Lo que dibujan los medidores: pico con caída, voces y la reducción del limitador.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeterView {
    /// Nivel de pico de las 8 pistas y el master, en amplitud.
    pub level: [f32; TRACKS + 1],
    pub voices: [u8; TRACKS],
    pub pool_voices: u8,
    /// Reducción del limitador en dB (≤ 0), con caída.
    pub limiter_db: f32,
}

/// Caída del pico por reporte (30 por segundo): ~1 dB, unos 30 dB/s.
const METER_FALL: f32 = 0.89;
/// Rango de los medidores dibujados.
pub const METER_FLOOR_DB: f32 = -48.0;
const METER_STEPS: f32 = 48.0;

/// Posición de un nivel en el medidor, 0–1 (−48 a 0 dBFS).
pub fn meter_fraction(level: f32) -> f32 {
    if level <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * level.log10();
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

impl MeterView {
    /// Suma un reporte del motor. Devuelve `true` si cambió algo que se ve.
    pub fn update(&mut self, m: &Meters) -> bool {
        let before = self.signature();
        for (shown, &peak) in self.level.iter_mut().zip(&m.peaks) {
            *shown = peak.max(*shown * METER_FALL);
            if *shown < 1.0e-4 {
                *shown = 0.0;
            }
        }
        self.voices = m.voices;
        self.pool_voices = m.pool_voices;
        let db = 20.0 * m.limiter_gain.max(1.0e-3).log10();
        self.limiter_db = db.min(self.limiter_db * METER_FALL);
        if self.limiter_db > -0.05 {
            self.limiter_db = 0.0;
        }
        self.signature() != before
    }

    /// Lo que se ve, cuantizado: si no cambia, no hace falta redibujar.
    fn signature(&self) -> ([u8; TRACKS + 1], [u8; TRACKS], u8, i16) {
        (
            self.level
                .map(|l| (meter_fraction(l) * METER_STEPS).round() as u8),
            self.voices,
            self.pool_voices,
            (self.limiter_db * 10.0).round() as i16,
        )
    }
}

fn delta(dir: Dir, fine: i32, coarse: i32, fast: bool) -> i32 {
    let d = match dir {
        Dir::Right => fine,
        Dir::Left => -fine,
        Dir::Up => coarse,
        Dir::Down => -coarse,
    };
    if fast { d * 4 } else { d }
}

fn nudge(v: u8, d: i32) -> u8 {
    (i32::from(v) + d).clamp(0, 127) as u8
}

/// Valor de un parámetro de canal como se muestra en la grilla.
pub fn channel_value(strip: &ChannelStrip, row: MixRow) -> String {
    match row {
        MixRow::Volume => format!("{:03}", strip.volume),
        MixRow::Pan => pan_label(strip.pan),
        MixRow::DelaySend => format!("{:03}", strip.delay_send),
        MixRow::ReverbSend => format!("{:03}", strip.reverb_send),
        MixRow::Mute => if strip.mute { "M" } else { "-" }.to_string(),
        MixRow::Solo => if strip.solo { "S" } else { "-" }.to_string(),
        _ => String::new(),
    }
}

pub fn master_gain_label(gain: i8) -> String {
    format!("{gain:+} dB")
}

/// Valor de un parámetro de efecto o del master.
pub fn fx_value(fx: &FxSettings, master: &MasterSettings, row: MixRow, item: usize) -> String {
    let d = &fx.delay;
    let r = &fx.reverb;
    let n = |v: u8| format!("{v:03}");
    match (row, item) {
        (MixRow::Delay, 0) => d.time.label().to_string(),
        (MixRow::Delay, 1) => n(d.feedback),
        (MixRow::Delay, 2) => n(d.tone),
        (MixRow::Delay, 3) => n(d.width),
        (MixRow::Delay, _) => n(d.return_level),
        (MixRow::Reverb, 0) => n(r.size),
        (MixRow::Reverb, 1) => n(r.damping),
        (MixRow::Reverb, 2) => n(r.predelay),
        (MixRow::Reverb, _) => n(r.return_level),
        (MixRow::Master, 0) => if master.limiter { "SÍ" } else { "NO" }.to_string(),
        (MixRow::Master, _) => master_gain_label(master.gain),
        _ => String::new(),
    }
}

fn db_text(v: u8) -> String {
    match level_db(v) {
        Some(db) => format!("{db:+.1} dB").replace('.', ","),
        None => "silencio".to_string(),
    }
}

impl App {
    pub(crate) fn mix_row(&self) -> MixRow {
        self.mix.row()
    }

    /// Gestos de la pantalla Mezcla que no son globales.
    pub(crate) fn handle_mix(&mut self, g: Gesture) {
        let last_row = MixRow::ALL.len() - 1;
        let row = self.mix_row();
        match g {
            Gesture::Move(Dir::Up) => self.set_mix_row(self.mix.row.saturating_sub(1)),
            Gesture::Move(Dir::Down) => self.set_mix_row((self.mix.row + 1).min(last_row)),
            Gesture::Move(dir) => self.move_mix_column(dir == Dir::Right, false),
            Gesture::Jump(Dir::Up) => self.set_mix_row(0),
            Gesture::Jump(Dir::Down) => self.set_mix_row(last_row),
            Gesture::Jump(dir) => self.move_mix_column(dir == Dir::Right, true),
            Gesture::Insert | Gesture::InsertNew => {
                if row.is_toggle() || (row == MixRow::Master && self.mix.item == 0) {
                    self.edit_mix(Dir::Right, false);
                }
            }
            Gesture::Edit { dir, fast } => self.edit_mix(dir, fast),
            Gesture::Delete => self.reset_mix_value(),
            _ => {}
        }
    }

    fn set_mix_row(&mut self, row: usize) {
        self.mix.row = row;
        let r = self.mix_row();
        if r != MixRow::Volume {
            if self.mix.master {
                self.track = TRACKS - 1;
            }
            self.mix.master = false;
        }
        if !r.is_channel() {
            self.mix.item = self.mix.item.min(r.items().len() - 1);
        }
    }

    /// ←/→: canal (o parámetro en las filas de efectos). Con B, al primero o al último.
    fn move_mix_column(&mut self, right: bool, jump: bool) {
        let row = self.mix_row();
        if !row.is_channel() {
            let last = row.items().len() - 1;
            self.mix.item = match (right, jump) {
                (true, true) => last,
                (false, true) => 0,
                (true, false) => (self.mix.item + 1).min(last),
                (false, false) => self.mix.item.saturating_sub(1),
            };
            return;
        }
        // La columna MASTER solo tiene volumen (la ganancia master).
        let has_master = row == MixRow::Volume;
        match (right, jump) {
            (true, true) if has_master => self.mix.master = true,
            (true, true) => self.track = TRACKS - 1,
            (false, true) => {
                self.mix.master = false;
                self.track = 0;
            }
            (true, false) if self.track == TRACKS - 1 && has_master => self.mix.master = true,
            (true, false) if !self.mix.master => self.track = (self.track + 1).min(TRACKS - 1),
            (false, false) if self.mix.master => self.mix.master = false,
            (false, false) => self.track = self.track.saturating_sub(1),
            _ => {}
        }
    }

    fn edit_mix(&mut self, dir: Dir, fast: bool) {
        let row = self.mix_row();
        let before = (
            self.project.tracks[self.track].mix,
            self.project.fx,
            self.project.master,
        );
        if self.mix.master && row == MixRow::Volume {
            self.nudge_master_gain(delta(dir, 1, 6, fast));
        } else if row.is_channel() {
            let strip = &mut self.project.tracks[self.track].mix;
            let d = delta(dir, 1, 10, fast);
            match row {
                MixRow::Volume => strip.volume = nudge(strip.volume, d),
                MixRow::Pan => strip.pan = (i32::from(strip.pan) + d).clamp(-64, 63) as i8,
                MixRow::DelaySend => strip.delay_send = nudge(strip.delay_send, d),
                MixRow::ReverbSend => strip.reverb_send = nudge(strip.reverb_send, d),
                MixRow::Mute => strip.mute = !strip.mute,
                _ => strip.solo = !strip.solo,
            }
        } else {
            let d = delta(dir, 1, 10, fast);
            let fx = &mut self.project.fx;
            match (row, self.mix.item) {
                (MixRow::Delay, 0) => {
                    // Lista: ±1 con cualquier dirección (spec-ui, principio 1).
                    let step = if matches!(dir, Dir::Right | Dir::Up) {
                        1
                    } else {
                        -1
                    };
                    let i = (fx.delay.time.index() as i32 + step)
                        .clamp(0, DelayTime::ALL.len() as i32 - 1);
                    fx.delay.time = DelayTime::ALL[i as usize];
                }
                (MixRow::Delay, 1) => fx.delay.feedback = nudge(fx.delay.feedback, d),
                (MixRow::Delay, 2) => fx.delay.tone = nudge(fx.delay.tone, d),
                (MixRow::Delay, 3) => fx.delay.width = nudge(fx.delay.width, d),
                (MixRow::Delay, _) => fx.delay.return_level = nudge(fx.delay.return_level, d),
                (MixRow::Reverb, 0) => fx.reverb.size = nudge(fx.reverb.size, d),
                (MixRow::Reverb, 1) => fx.reverb.damping = nudge(fx.reverb.damping, d),
                (MixRow::Reverb, 2) => fx.reverb.predelay = nudge(fx.reverb.predelay, d),
                (MixRow::Reverb, _) => {
                    fx.reverb.return_level = nudge(fx.reverb.return_level, d);
                }
                (_, 0) => self.project.master.limiter = !self.project.master.limiter,
                _ => self.nudge_master_gain(delta(dir, 1, 6, fast)),
            }
        }
        let after = (
            self.project.tracks[self.track].mix,
            self.project.fx,
            self.project.master,
        );
        if after != before {
            self.modified = true;
        }
    }

    fn nudge_master_gain(&mut self, d: i32) {
        let m = &mut self.project.master;
        m.gain = (i32::from(m.gain) + d)
            .clamp(i32::from(MASTER_GAIN_MIN), i32::from(MASTER_GAIN_MAX)) as i8;
    }

    /// A + B: el valor por defecto del parámetro bajo el cursor.
    fn reset_mix_value(&mut self) {
        let row = self.mix_row();
        let before = (
            self.project.tracks[self.track].mix,
            self.project.fx,
            self.project.master,
        );
        let strip_default = ChannelStrip::default();
        let fx_default = FxSettings::default();
        if self.mix.master && row == MixRow::Volume {
            self.project.master.gain = 0;
        } else if row.is_channel() {
            let strip = &mut self.project.tracks[self.track].mix;
            match row {
                MixRow::Volume => strip.volume = strip_default.volume,
                MixRow::Pan => strip.pan = 0,
                MixRow::DelaySend => strip.delay_send = 0,
                MixRow::ReverbSend => strip.reverb_send = 0,
                MixRow::Mute => strip.mute = false,
                _ => strip.solo = false,
            }
        } else {
            let (fx, d, r) = (&mut self.project.fx, fx_default.delay, fx_default.reverb);
            match (row, self.mix.item) {
                (MixRow::Delay, 0) => fx.delay.time = d.time,
                (MixRow::Delay, 1) => fx.delay.feedback = d.feedback,
                (MixRow::Delay, 2) => fx.delay.tone = d.tone,
                (MixRow::Delay, 3) => fx.delay.width = d.width,
                (MixRow::Delay, _) => fx.delay.return_level = d.return_level,
                (MixRow::Reverb, 0) => fx.reverb.size = r.size,
                (MixRow::Reverb, 1) => fx.reverb.damping = r.damping,
                (MixRow::Reverb, 2) => fx.reverb.predelay = r.predelay,
                (MixRow::Reverb, _) => fx.reverb.return_level = r.return_level,
                (_, 0) => self.project.master.limiter = MasterSettings::default().limiter,
                _ => self.project.master.gain = 0,
            }
        }
        let after = (
            self.project.tracks[self.track].mix,
            self.project.fx,
            self.project.master,
        );
        if after != before {
            self.modified = true;
        }
    }

    /// Línea de información del parámetro bajo el cursor (nombre, canal, valor y unidades).
    pub fn mix_info(&self) -> String {
        let row = self.mix_row();
        let track = &self.project.tracks[self.track];
        let channel = format!("PISTA {} {}", self.track + 1, track.name);
        let strip = &track.mix;
        let fx = &self.project.fx;
        if self.mix.master && row == MixRow::Volume {
            return format!(
                "GANANCIA MASTER  {}",
                master_gain_label(self.project.master.gain)
            );
        }
        if row.is_channel() {
            let value = match row {
                MixRow::Volume => format!("{:03} ({})", strip.volume, db_text(strip.volume)),
                MixRow::Pan => pan_label(strip.pan),
                MixRow::DelaySend => {
                    format!("{:03} ({})", strip.delay_send, db_text(strip.delay_send))
                }
                MixRow::ReverbSend => {
                    format!("{:03} ({})", strip.reverb_send, db_text(strip.reverb_send))
                }
                MixRow::Mute => if strip.mute { "SÍ" } else { "NO" }.to_string(),
                _ => if strip.solo { "SÍ" } else { "NO" }.to_string(),
            };
            let name = match row {
                MixRow::DelaySend => "ENVÍO A DELAY".to_string(),
                MixRow::ReverbSend => "ENVÍO A REVERB".to_string(),
                r => r.label().to_uppercase(),
            };
            return format!("{name} {channel}  {value}");
        }
        let item = self.mix.item.min(row.items().len() - 1);
        let name = row.items()[item].1;
        let value = fx_value(fx, &self.project.master, row, item);
        let unit = match (row, item) {
            (MixRow::Delay, 0) => format!(
                "({} ms a {:.0} BPM)",
                (fx.delay.time.seconds(self.project.tempo_bpm) * 1000.0).round(),
                self.project.tempo_bpm
            ),
            (MixRow::Delay, 1) => format!(
                "({:.0} %)",
                crate::mix::delay_feedback(fx.delay.feedback) * 100.0
            ),
            (MixRow::Delay, 2) => format!("(pasa-bajos {:.0} Hz)", delay_tone_hz(fx.delay.tone)),
            (MixRow::Delay, 3) => "(0 estéreo, 127 ping-pong)".to_string(),
            (MixRow::Delay, _) => format!("({})", db_text(fx.delay.return_level)),
            (MixRow::Reverb, 0) => {
                format!("(cola de {:.1} s)", reverb_rt60(fx.reverb.size)).replace('.', ",")
            }
            (MixRow::Reverb, 1) => format!(
                "(agudos desde {:.1} kHz)",
                reverb_damping_hz(fx.reverb.damping) / 1000.0
            )
            .replace('.', ","),
            (MixRow::Reverb, 2) => {
                format!("({:.0} ms)", reverb_predelay_s(fx.reverb.predelay) * 1000.0)
            }
            (MixRow::Reverb, _) => format!("({})", db_text(fx.reverb.return_level)),
            (_, 0) => "(picos > 0 dBFS antes del soft clip)".to_string(),
            _ => String::new(),
        };
        format!("{name}  {value} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::model::Project;

    fn mix_app() -> App {
        let mut a = App::new(Project::new_default());
        a.screen = Screen::Mix;
        a
    }

    fn edit(a: &mut App, dir: Dir) {
        a.handle(Gesture::Edit { dir, fast: false }, 0);
    }

    #[test]
    fn channel_values_are_edited_with_a_and_the_cross() {
        let mut a = mix_app();
        a.track = 2;
        edit(&mut a, Dir::Down);
        assert_eq!(a.project.tracks[2].mix.volume, 90);
        assert!(a.modified);
        a.handle(Gesture::Move(Dir::Down), 0);
        edit(&mut a, Dir::Right);
        edit(&mut a, Dir::Up);
        assert_eq!(a.project.tracks[2].mix.pan, 11);
        a.handle(Gesture::Move(Dir::Down), 0);
        edit(&mut a, Dir::Up);
        assert_eq!(a.project.tracks[2].mix.delay_send, 10);
        a.handle(Gesture::Delete, 0);
        assert_eq!(a.project.tracks[2].mix.delay_send, 0);
        // Mute con A (y con Select + X, global).
        a.handle(Gesture::Jump(Dir::Down), 0);
        for _ in 0..4 {
            a.handle(Gesture::Move(Dir::Up), 0);
        }
        assert_eq!(a.mix_row(), MixRow::Mute);
        a.handle(Gesture::Insert, 0);
        assert!(a.project.tracks[2].mix.mute);
        a.handle(Gesture::Mute, 0);
        assert!(!a.project.tracks[2].mix.mute);
        // Los extremos se respetan.
        a.mix.row = 1;
        for _ in 0..20 {
            a.handle(
                Gesture::Edit {
                    dir: Dir::Down,
                    fast: true,
                },
                0,
            );
        }
        assert_eq!(a.project.tracks[2].mix.pan, -64);
    }

    #[test]
    fn b_jumps_instead_of_leaving_and_select_up_returns() {
        let mut a = mix_app();
        a.origin = Screen::Phrase;
        a.handle(Gesture::Cancel, 0);
        a.handle(Gesture::Jump(Dir::Down), 0);
        assert_eq!((a.screen, a.mix_row()), (Screen::Mix, MixRow::Master));
        a.handle(Gesture::Screen(Dir::Up), 0);
        assert_eq!(a.screen, Screen::Phrase);
    }

    #[test]
    fn cursor_reaches_master_only_on_the_volume_row() {
        let mut a = mix_app();
        a.track = 7;
        a.handle(Gesture::Move(Dir::Right), 0);
        assert!(a.mix.master);
        edit(&mut a, Dir::Up);
        assert_eq!(a.project.master.gain, 6);
        assert!(a.mix_info().starts_with("GANANCIA MASTER  +6 dB"));
        a.handle(Gesture::Move(Dir::Down), 0);
        assert!(!a.mix.master);
        assert_eq!(a.track, 7);
        a.handle(Gesture::Move(Dir::Right), 0);
        assert_eq!(a.track, 7, "en paneo no hay columna MASTER");
        a.handle(Gesture::Jump(Dir::Left), 0);
        assert_eq!(a.track, 0);
    }

    #[test]
    fn fx_rows_edit_their_parameters() {
        let mut a = mix_app();
        a.handle(Gesture::Jump(Dir::Down), 0);
        assert_eq!(a.mix_row(), MixRow::Master);
        a.handle(Gesture::Insert, 0);
        assert!(!a.project.master.limiter);
        a.handle(Gesture::Move(Dir::Up), 0);
        a.handle(Gesture::Move(Dir::Up), 0);
        assert_eq!(a.mix_row(), MixRow::Delay);
        edit(&mut a, Dir::Right);
        assert_eq!(a.project.fx.delay.time, DelayTime::Quarter);
        assert!(
            a.mix_info().contains("500 ms a 120 BPM"),
            "{}",
            a.mix_info()
        );
        a.handle(Gesture::Jump(Dir::Right), 0);
        edit(&mut a, Dir::Down);
        assert_eq!(a.project.fx.delay.return_level, 70);
        a.handle(Gesture::Delete, 0);
        assert_eq!(a.project.fx.delay.return_level, 80);
        a.handle(Gesture::Move(Dir::Down), 0);
        assert_eq!(a.mix.item, 3, "la reverb tiene 4 parámetros");
        a.handle(Gesture::Jump(Dir::Left), 0);
        edit(&mut a, Dir::Up);
        assert_eq!(a.project.fx.reverb.size, 70);
        assert!(a.mix_info().starts_with("TAMAÑO DE LA REVERB  070"));
    }

    #[test]
    fn meters_fall_and_report_visible_changes() {
        let mut v = MeterView::default();
        let mut m = Meters {
            limiter_gain: 1.0,
            ..Meters::default()
        };
        assert!(!v.update(&m), "en silencio no cambia nada");
        m.peaks[0] = 0.5;
        m.limiter_gain = 0.8;
        assert!(v.update(&m));
        assert!((v.limiter_db + 1.94).abs() < 0.01);
        m.peaks[0] = 0.0;
        m.limiter_gain = 1.0;
        let mut reports = 0;
        while v.level[0] > 0.0 {
            v.update(&m);
            reports += 1;
        }
        // De −6 dB a −80 dB a ~1 dB por reporte.
        assert!((60..90).contains(&reports), "{reports}");
        assert_eq!(v.limiter_db, 0.0);
        assert_eq!(meter_fraction(1.0), 1.0);
        assert_eq!(meter_fraction(0.0), 0.0);
    }
}
