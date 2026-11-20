//! Motor de sonido (H2) y secuenciador (H3, H5): comandos de la interfaz, cabezales, bolsa de voces,
//! instrumentos, mezcla y master.
//!
//! No depende de cpal: `audio.rs` lo llama desde el callback y `offline.rs` lo usa para
//! renderizar a WAV y medir. Nada en `handle` ni en `render` asigna memoria, bloquea ni hace
//! I/O. Diseño y costos en docs/solutions/groovebox/dsp.md.

pub mod drum;
pub mod dsp;
pub mod load;
pub mod pool;
pub mod sequencer;
pub mod synth;

use drum::{DrumSetup, DrumVoice};
use dsp::{Smoother, soft_clip};
pub use load::{LoadMeter, LoadReport};
use pool::{Source, Voice, VoicePool, VoiceState};
pub use sequencer::{HeadPos, PlayMode, Playhead};
use sequencer::{Sequencer, Trigger, samples_per_step};
use synth::{SynthSetup, SynthVoice};

use crate::instrument::{Instrument, MAX_VOICES, Patch, default_kit, velocity_gain, volume_gain};
use crate::model::{
    ChainEntry, ChainId, GATE_INF, NoteSet, PhraseId, Step, TICKS_PER_STEP, TRACKS,
};

pub const MAX_NOTES: usize = 4;
/// Subbloque interno (ADR 0005): las modulaciones se recalculan cada ≤ 32 muestras. Los
/// subbloques están alineados a la muestra absoluta (no al callback) y se cortan en cada paso
/// del secuenciador: con buffers múltiplos de 32 el audio no depende del tamaño de buffer. Con
/// otros tamaños el fin del callback corta un subbloque más (los disparos siguen exactos).
pub const BLOCK: usize = 32;
/// Largo que se le da a una audición con largo INF (no hay "próxima nota"): 16 pasos.
const AUDITION_INF_TICKS: u32 = 16 * TICKS_PER_STEP as u32;
const TICKS_PER_BEAT: f32 = 4.0 * TICKS_PER_STEP as f32;
/// Suavizado de volúmenes (pista y master).
const GAIN_TAU: f32 = 0.010;
const MASTER_GAIN: f32 = 1.0;

/// Mensajes de la interfaz al motor (cola `rtrb`). Todo `Copy`: nada que liberar en el callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    SetInstrument {
        track: u8,
        instrument: Instrument,
    },
    SetTempo(f32),
    SetMix {
        track: u8,
        mute: bool,
        solo: bool,
    },
    SetStep {
        phrase: PhraseId,
        step: u8,
        value: Step,
    },
    SetChainEntry {
        chain: ChainId,
        pos: u8,
        value: Option<ChainEntry>,
    },
    SetSongCell {
        row: u8,
        track: u8,
        chain: Option<ChainId>,
    },
    Play(PlayMode),
    Stop,
    /// Escuchar notas en una pista con su instrumento; `len` dice cuántas de `notes` valen
    /// (de grave a aguda, como las da `Chord::notes`).
    Audition {
        track: u8,
        notes: [u8; MAX_NOTES],
        len: u8,
        velocity: u8,
        gate_ticks: u16,
    },
}

/// Mensajes del motor a la interfaz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Load(LoadReport),
    Playhead(Playhead),
}

#[derive(Clone, Copy, Debug)]
enum Setup {
    Drum(DrumSetup),
    Synth(SynthSetup),
    Silent,
}

struct TrackState {
    instrument: Instrument,
    setup: Setup,
    gain: Smoother,
    mute: bool,
    solo: bool,
}

impl TrackState {
    fn new(instrument: Instrument, sample_rate: f32) -> Self {
        Self {
            instrument,
            setup: prepare(&instrument, sample_rate),
            gain: Smoother::new(volume_gain(instrument.volume), GAIN_TAU, sample_rate),
            mute: false,
            solo: false,
        }
    }
}

fn prepare(instrument: &Instrument, sample_rate: f32) -> Setup {
    match &instrument.patch {
        Patch::Drum(p) => Setup::Drum(DrumSetup::new(p, sample_rate)),
        Patch::Synth(p) => Setup::Synth(SynthSetup::new(p, sample_rate)),
        Patch::Sampler => Setup::Silent,
    }
}

pub struct Engine {
    sample_rate: f32,
    tempo_bpm: f32,
    tracks: [TrackState; TRACKS],
    pool: VoicePool,
    /// Un buffer por pista; se mantiene en cero entre bloques.
    bufs: [[f32; BLOCK]; TRACKS],
    mix: [f32; BLOCK],
    master: Smoother,
    seq: Sequencer,
    /// Muestras renderizadas desde que se creó el motor: el reloj del secuenciador.
    frame: u64,
    /// Máximo de voces en uso desde la última lectura (se mide en cada subbloque).
    voices_peak: usize,
    /// Notas disparadas desde la última lectura (audición y secuenciador).
    notes_started: u32,
    /// Pico absoluto de la salida desde la última lectura (para tests y el render offline).
    pub peak: f32,
    /// (muestra, pista, nota raíz; 0 = OFF) de cada disparo del secuenciador, sin pasar de la
    /// capacidad reservada.
    #[cfg(test)]
    pub trigger_log: Vec<(u64, u8, u8)>,
}

impl Engine {
    /// Arranca con el kit por defecto: suena aunque la interfaz todavía no mandó instrumentos.
    pub fn new(sample_rate: f32) -> Self {
        let kit = default_kit();
        let tempo_bpm = 120.0;
        Self {
            sample_rate,
            tempo_bpm,
            tracks: std::array::from_fn(|t| TrackState::new(kit[t].1, sample_rate)),
            pool: VoicePool::new(sample_rate),
            bufs: [[0.0; BLOCK]; TRACKS],
            mix: [0.0; BLOCK],
            master: Smoother::new(MASTER_GAIN, GAIN_TAU, sample_rate),
            seq: Sequencer::new(samples_per_step(sample_rate, tempo_bpm)),
            frame: 0,
            voices_peak: 0,
            notes_started: 0,
            peak: 0.0,
            #[cfg(test)]
            trigger_log: Vec::with_capacity(4096),
        }
    }

    pub fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::SetInstrument { track, instrument } => {
                let Some(state) = self.tracks.get_mut(usize::from(track)) else {
                    return;
                };
                // Las voces que ya suenan conservan su copia de los parámetros.
                state.instrument = instrument;
                state.setup = prepare(&instrument, self.sample_rate);
                state.gain.target = volume_gain(instrument.volume);
            }
            Command::SetTempo(bpm) => {
                if bpm.is_finite() {
                    self.tempo_bpm = bpm.clamp(20.0, 999.0);
                    let sps = samples_per_step(self.sample_rate, self.tempo_bpm);
                    self.seq.set_samples_per_step(sps, self.frame);
                }
            }
            Command::SetMix { track, mute, solo } => {
                if let Some(state) = self.tracks.get_mut(usize::from(track)) {
                    state.mute = mute;
                    state.solo = solo;
                    self.update_audible();
                }
            }
            Command::SetStep {
                phrase,
                step,
                value,
            } => {
                self.seq.data.set_step(phrase, usize::from(step), value);
            }
            Command::SetChainEntry { chain, pos, value } => {
                self.seq
                    .data
                    .set_chain_entry(chain, usize::from(pos), value);
            }
            Command::SetSongCell { row, track, chain } => {
                self.seq
                    .data
                    .set_song_cell(usize::from(row), usize::from(track), chain);
            }
            Command::Play(mode) => {
                if self.seq.is_playing() {
                    self.release_all();
                }
                self.seq.play(mode, self.frame);
            }
            Command::Stop => {
                if self.seq.is_playing() {
                    self.seq.stop();
                    self.release_all();
                }
            }
            Command::Audition {
                track,
                notes,
                len,
                velocity,
                gate_ticks,
            } => {
                let len = usize::from(len).min(MAX_NOTES);
                let gate = if gate_ticks == GATE_INF {
                    AUDITION_INF_TICKS
                } else {
                    u32::from(gate_ticks)
                };
                let gate = self.ticks_to_samples(gate);
                self.note_on(usize::from(track), &notes[..len], velocity, Some(gate));
            }
        }
    }

    pub fn ticks_to_samples(&self, ticks: u32) -> u32 {
        let per_tick = self.sample_rate * 60.0 / (self.tempo_bpm * TICKS_PER_BEAT);
        (ticks as f32 * per_tick).round() as u32
    }

    fn update_audible(&mut self) {
        let any_solo = self.tracks.iter().any(|t| t.solo);
        let mut audible = [true; TRACKS];
        for (t, (a, state)) in audible.iter_mut().zip(&self.tracks).enumerate() {
            *a = !state.mute && (!any_solo || state.solo);
            // Lo que queda sostenido en una pista que se apaga se suelta (sigue su relajación).
            if !*a {
                self.pool.release_track(t as u8, false);
            }
        }
        self.seq.set_audible(audible);
    }

    fn release_all(&mut self) {
        for t in 0..TRACKS {
            self.pool.release_track(t as u8, false);
        }
    }

    /// Dispara `notes` (de grave a aguda) en `track`. Si la pista tiene menos voces que notas,
    /// se descartan las más agudas. La percusión toca solo la primera y no usa el gate.
    /// `gate_samples = None` es INF: la nota sigue hasta la próxima de la pista.
    pub fn note_on(&mut self, track: usize, notes: &[u8], velocity: u8, gate_samples: Option<u32>) {
        let Some(state) = self.tracks.get(track) else {
            return;
        };
        let Some(&root) = notes.first() else {
            return;
        };
        let inst = state.instrument;
        let setup = state.setup;
        let gain = velocity_gain(velocity, inst.velocity_amount);
        let limit = inst.voices.clamp(1, MAX_VOICES);
        let track_id = track as u8;

        let notes = match setup {
            Setup::Silent => return,
            Setup::Drum(_) => &notes[..1],
            Setup::Synth(s) => {
                // Mono: legato sobre la voz que ya suena (la envolvente sube desde donde está).
                if limit == 1
                    && let Some(v) = self.pool.newest_of_track_mut(track_id)
                    && let Source::Synth(synth) = &mut v.source
                {
                    synth.legato(&s, root, gain);
                    v.note = root;
                    v.state = VoiceState::Held;
                    v.gate = gate_samples;
                    self.notes_started += 1;
                    return;
                }
                // INF dura hasta la próxima nota de la pista: esa nota suelta las anteriores.
                self.pool.release_track(track_id, true);
                &notes[..notes.len().min(usize::from(limit))]
            }
        };

        let trigger = self.pool.next_serial();
        for &note in notes {
            let slot = self.pool.allocate(track_id, limit, trigger);
            let age = self.pool.next_serial();
            let (source, state, gate) = match setup {
                Setup::Drum(d) => {
                    let seed = (age as u32) ^ (u32::from(note) << 16);
                    let v = DrumVoice::new(&d, note, gain, seed);
                    (Source::Drum(v), VoiceState::Released, None)
                }
                Setup::Synth(s) => {
                    let v = SynthVoice::new(&s, note, gain);
                    (Source::Synth(v), VoiceState::Held, gate_samples)
                }
                Setup::Silent => unreachable!("se descartó arriba"),
            };
            self.pool.start(
                slot,
                Voice {
                    state,
                    track: track_id,
                    note,
                    age,
                    trigger,
                    gate,
                    rendered: false,
                    source,
                },
            );
            self.notes_started += 1;
        }
    }

    /// Dispara el paso del secuenciador que cae en la muestra actual.
    fn fire_step(&mut self) {
        let triggers = self.seq.fire();
        for (t, trigger) in triggers.into_iter().enumerate() {
            let Some(trigger) = trigger else { continue };
            #[cfg(test)]
            if self.trigger_log.len() < self.trigger_log.capacity() {
                let root = match trigger {
                    Trigger::Notes { root, .. } => root,
                    Trigger::Off => 0,
                };
                self.trigger_log.push((self.frame, t as u8, root));
            }
            match trigger {
                Trigger::Off => self.pool.release_track(t as u8, false),
                Trigger::Notes {
                    root,
                    chord,
                    velocity,
                    gate_ticks,
                } => {
                    let notes = match self.tracks[t].setup {
                        Setup::Drum(_) => NoteSet::single(root),
                        _ => chord.notes(root),
                    };
                    let gate = (gate_ticks != GATE_INF)
                        .then(|| self.ticks_to_samples(u32::from(gate_ticks)));
                    self.note_on(t, notes.as_slice(), velocity, gate);
                }
            }
        }
    }

    #[cfg(test)]
    pub fn is_playing(&self) -> bool {
        self.seq.is_playing()
    }

    /// Posición de los cabezales, si cambió desde la última llamada.
    pub fn take_playhead(&mut self) -> Option<Playhead> {
        self.seq.take_playhead()
    }

    /// Máximo de voces en uso desde la última llamada.
    pub fn take_voices_peak(&mut self) -> usize {
        std::mem::take(&mut self.voices_peak)
    }

    /// Notas disparadas desde la última llamada.
    pub fn take_notes_started(&mut self) -> u32 {
        std::mem::take(&mut self.notes_started)
    }

    pub fn voices_in_use(&self) -> usize {
        self.pool.in_use()
    }

    pub fn pool(&self) -> &VoicePool {
        &self.pool
    }

    #[cfg(test)]
    pub fn song_data(&self) -> &sequencer::SongData {
        &self.seq.data
    }

    #[cfg(test)]
    pub fn is_silent(&self) -> bool {
        self.pool.is_idle()
    }

    /// Llena `out` (intercalado, `channels` canales). Por ahora todo es mono: el mismo valor
    /// en todos los canales (el paneo llega con la Mezcla, H9). Los pasos del secuenciador se
    /// disparan en su muestra exacta dentro del buffer.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let mut pos = 0;
        while pos < frames {
            while let Some(due) = self.seq.next_step_frame()
                && due <= self.frame
            {
                self.fire_step();
            }
            let mut n = (BLOCK - (self.frame % BLOCK as u64) as usize).min(frames - pos);
            if let Some(due) = self.seq.next_step_frame() {
                n = n.min((due - self.frame) as usize);
            }
            let chunk = &mut out[pos * channels..(pos + n) * channels];
            let voices = self.pool.in_use();
            self.voices_peak = self.voices_peak.max(voices);
            if voices == 0 {
                chunk.fill(0.0);
                self.master.settle();
                for t in &mut self.tracks {
                    t.gain.settle();
                }
            } else {
                self.render_block(n);
                for (frame, &x) in chunk.chunks_exact_mut(channels).zip(&self.mix[..n]) {
                    frame.fill(x);
                }
            }
            pos += n;
            self.frame += n as u64;
        }
        out[frames * channels..].fill(0.0);
    }

    fn render_block(&mut self, n: usize) {
        let mask = self.pool.render(&mut self.bufs, n);
        let mix = &mut self.mix[..n];
        mix.fill(0.0);
        for (t, (state, buf)) in self.tracks.iter_mut().zip(&mut self.bufs).enumerate() {
            if mask & (1 << t) == 0 {
                state.gain.settle();
                continue;
            }
            let buf = &mut buf[..n];
            if state.gain.is_settled() {
                let g = state.gain.value;
                for (m, x) in mix.iter_mut().zip(buf.iter()) {
                    *m += x * g;
                }
            } else {
                for (m, x) in mix.iter_mut().zip(buf.iter()) {
                    *m += x * state.gain.next();
                }
            }
            buf.fill(0.0);
        }
        let mut peak = self.peak;
        for m in mix.iter_mut() {
            *m = soft_clip(*m * self.master.next());
            peak = peak.max(m.abs());
        }
        self.peak = peak;
    }
}

#[cfg(test)]
mod seq_tests;
#[cfg(test)]
mod tests;
