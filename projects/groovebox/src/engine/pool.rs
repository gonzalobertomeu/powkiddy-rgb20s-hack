//! Bolsa global de voces (ADR 0007): 16 voces compartidas por las 8 pistas, más 4 ranuras de
//! fundido donde las voces robadas se apagan en 5 ms. Todo en arrays fijos.

use super::BLOCK;
use super::drum::DrumVoice;
use super::synth::{SynthVoice, VoiceMod};
use crate::model::TRACKS;

pub const POOL_VOICES: usize = 16;
// ADR 0013: una ranura por voz, para que un cambio de acorde en todas las pistas no corte en seco.
pub const FADE_SLOTS: usize = POOL_VOICES;
pub const FADE_SECONDS: f32 = 0.005;

#[derive(Clone, Copy, Debug)]
pub enum Source {
    Drum(DrumVoice),
    Synth(SynthVoice),
}

impl Source {
    fn render(&mut self, out: &mut [f32], m: VoiceMod) -> bool {
        match self {
            Source::Drum(v) => v.render(out),
            Source::Synth(v) => v.render(out, m),
        }
    }

    fn release(&mut self) {
        if let Source::Synth(v) = self {
            v.release();
        }
    }

    /// Ya no se oye (la percusión no se puede saber barato: se asume que sí).
    fn is_inaudible(&self) -> bool {
        match self {
            Source::Drum(_) => false,
            Source::Synth(v) => v.level() < super::dsp::SILENCE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceState {
    /// Gate abierto (o esperando note-off).
    Held,
    /// En relajación: la percusión está siempre acá (es one-shot).
    Released,
}

#[derive(Clone, Copy, Debug)]
pub struct Voice {
    pub state: VoiceState,
    pub track: u8,
    pub note: u8,
    /// Orden de inicio: menor = más vieja.
    pub age: u64,
    /// Disparo al que pertenece (las notas de un mismo acorde comparten disparo).
    pub trigger: u64,
    /// Muestras hasta el note-off.
    pub gate: Option<u32>,
    /// Ya sonó al menos un bloque. Una voz que todavía no sonó se puede descartar sin fundido.
    pub rendered: bool,
    pub source: Source,
}

#[derive(Clone, Copy, Debug)]
struct Fade {
    track: u8,
    gain: f32,
    source: Source,
}

pub struct VoicePool {
    voices: [Option<Voice>; POOL_VOICES],
    fades: [Option<Fade>; FADE_SLOTS],
    fade_step: f32,
    serial: u64,
    scratch: [f32; BLOCK],
    /// Fundidos que se cortaron en seco por falta de ranuras (diagnóstico).
    pub hard_cuts: u32,
}

impl VoicePool {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            voices: [None; POOL_VOICES],
            fades: [None; FADE_SLOTS],
            fade_step: 1.0 / (FADE_SECONDS * sample_rate),
            serial: 0,
            scratch: [0.0; BLOCK],
            hard_cuts: 0,
        }
    }

    pub fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    pub fn voices(&self) -> impl Iterator<Item = &Voice> {
        self.voices.iter().flatten()
    }

    pub fn fading(&self) -> usize {
        self.fades.iter().flatten().count()
    }

    /// Voces sonando, incluidas las que se están fundiendo (máximo 20).
    pub fn in_use(&self) -> usize {
        self.voices().count() + self.fading()
    }

    /// Voces (sin contar los fundidos) de cada pista, en `counts`.
    pub fn count_by_track(&self, counts: &mut [u8; TRACKS]) {
        counts.fill(0);
        for v in self.voices() {
            counts[usize::from(v.track)] += 1;
        }
    }

    #[cfg(test)]
    pub fn is_idle(&self) -> bool {
        self.in_use() == 0
    }

    /// La voz más nueva de la pista (para el legato del modo mono).
    pub fn newest_of_track_mut(&mut self, track: u8) -> Option<&mut Voice> {
        self.voices
            .iter_mut()
            .flatten()
            .filter(|v| v.track == track)
            .max_by_key(|v| v.age)
    }

    fn oldest(&self, accept: impl Fn(&Voice) -> bool) -> Option<usize> {
        self.voices
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().filter(|v| accept(v)).map(|v| (i, v.age)))
            .min_by_key(|&(_, age)| age)
            .map(|(i, _)| i)
    }

    /// Pista con más voces, sin contar las del disparo actual.
    fn busiest_track(&self, trigger: u64) -> Option<u8> {
        let mut counts = [0u8; TRACKS];
        for v in self.voices().filter(|v| v.trigger != trigger) {
            counts[usize::from(v.track)] += 1;
        }
        let (track, &n) = counts.iter().enumerate().max_by_key(|&(_, n)| n)?;
        (n > 0).then_some(track as u8)
    }

    /// Devuelve una ranura libre para una nota nueva de `track`, robando según el ADR 0007:
    /// 1. si la pista ya usa `limit` voces, su voz más vieja;
    /// 2. si no, una voz libre;
    /// 3. con la bolsa llena: la más vieja en relajación, la más vieja de la pista que más voces
    ///    usa, o la más vieja de todas.
    ///
    /// Nunca elige una voz del mismo `trigger` (el acorde que se está disparando) si hay otra.
    pub fn allocate(&mut self, track: u8, limit: u8, trigger: u64) -> usize {
        let other = |v: &Voice| v.trigger != trigger;
        // Si se bajó el límite de la pista mientras sonaba, puede haber más de `limit`: se roban
        // las más viejas hasta dejar lugar para esta (a lo sumo 16 vueltas).
        let mut used = self.voices().filter(|v| v.track == track).count();
        let mut freed = None;
        while used >= usize::from(limit)
            && let Some(i) = self.oldest(|v| v.track == track && other(v))
        {
            self.steal(i);
            freed = Some(i);
            used -= 1;
        }
        if let Some(i) = freed {
            return i;
        }
        if let Some(i) = self.voices.iter().position(Option::is_none) {
            return i;
        }
        let i = self
            .oldest(|v| v.state == VoiceState::Released && other(v))
            .or_else(|| {
                let busiest = self.busiest_track(trigger)?;
                self.oldest(|v| v.track == busiest && other(v))
            })
            .or_else(|| self.oldest(other))
            .or_else(|| self.oldest(|_| true))
            .unwrap_or(0);
        self.steal(i);
        i
    }

    /// Pasa la voz a una ranura de fundido. Sin ranuras libres, se pisa la que lleva más tiempo
    /// fundiéndose (la de menor ganancia): ese corte sí puede hacer click, y se cuenta.
    fn steal(&mut self, i: usize) {
        let Some(v) = self.voices[i].take() else {
            return;
        };
        // Sin señal no hay discontinuidad: no gasta una ranura de fundido.
        if !v.rendered || v.source.is_inaudible() {
            return;
        }
        let fade = Fade {
            track: v.track,
            gain: 1.0,
            source: v.source,
        };
        if let Some(slot) = self.fades.iter_mut().find(|f| f.is_none()) {
            *slot = Some(fade);
            return;
        }
        let slot = self
            .fades
            .iter_mut()
            .min_by(|a, b| {
                let g = |f: &Option<Fade>| f.as_ref().map_or(0.0, |f| f.gain);
                g(a).total_cmp(&g(b))
            })
            .expect("FADE_SLOTS > 0");
        *slot = Some(fade);
        self.hard_cuts += 1;
    }

    pub fn start(&mut self, slot: usize, voice: Voice) {
        self.voices[slot] = Some(voice);
    }

    /// Note-off a las voces sostenidas de `track` (todas, o solo las de largo INF, que no
    /// tienen gate). Siguen sonando su relajación.
    pub fn release_track(&mut self, track: u8, only_untimed: bool) {
        for v in self.voices.iter_mut().flatten() {
            if v.track == track
                && v.state == VoiceState::Held
                && (!only_untimed || v.gate.is_none())
            {
                v.state = VoiceState::Released;
                v.gate = None;
                v.source.release();
            }
        }
    }

    /// Suma cada voz en el buffer de su pista (`n` ≤ BLOCK muestras). Aplica los note-off en su
    /// muestra exacta. `mods` es la modulación de los sticks de cada pista. Devuelve la máscara
    /// de pistas que recibieron señal.
    pub fn render(
        &mut self,
        bufs: &mut [[f32; BLOCK]; TRACKS],
        n: usize,
        mods: &[VoiceMod; TRACKS],
    ) -> u8 {
        let mut mask = 0u8;
        for slot in &mut self.voices {
            let Some(v) = slot else { continue };
            let buf = &mut bufs[usize::from(v.track)][..n];
            mask |= 1 << v.track;
            v.rendered = true;
            let m = mods[usize::from(v.track)];
            let alive = match v.gate {
                Some(g) if (g as usize) < n => {
                    let (a, b) = buf.split_at_mut(g as usize);
                    v.source.render(a, m);
                    v.gate = None;
                    v.state = VoiceState::Released;
                    v.source.release();
                    v.source.render(b, m)
                }
                Some(g) => {
                    v.gate = Some(g - n as u32);
                    v.source.render(buf, m)
                }
                None => v.source.render(buf, m),
            };
            if !alive {
                *slot = None;
            }
        }
        let step = self.fade_step;
        for slot in &mut self.fades {
            let Some(f) = slot else { continue };
            let scratch = &mut self.scratch[..n];
            scratch.fill(0.0);
            let alive = f.source.render(scratch, mods[usize::from(f.track)]);
            let buf = &mut bufs[usize::from(f.track)][..n];
            mask |= 1 << f.track;
            for (o, &s) in buf.iter_mut().zip(scratch.iter()) {
                f.gain = (f.gain - step).max(0.0);
                *o += s * f.gain;
            }
            if !alive || f.gain <= 0.0 {
                *slot = None;
            }
        }
        mask
    }
}
