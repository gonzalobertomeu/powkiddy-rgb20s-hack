//! Motor de sonido (H2) y secuenciador (H3, H5): comandos de la interfaz, cabezales, bolsa de voces,
//! instrumentos, mezcla estéreo con envíos a delay y reverb, y master con limitador (H9).
//!
//! No depende de cpal: `audio.rs` lo llama desde el callback y `offline.rs` lo usa para
//! renderizar a WAV y medir. Nada en `handle` ni en `render` asigna memoria, bloquea ni hace
//! I/O. Diseño y costos en docs/solutions/groovebox/dsp.md.

pub mod drum;
pub mod dsp;
pub mod fx;
pub mod load;
pub mod pool;
pub mod sequencer;
pub mod synth;

use drum::{DrumSetup, DrumVoice};
use dsp::soft_clip;
use fx::{BusFilter, Delay, Limiter, Ramp, Reverb, mix_into};
pub use load::{LoadMeter, LoadReport};
use pool::{Source, Voice, VoicePool, VoiceState};
pub use sequencer::{HeadPos, PlayMode, Playhead};
use sequencer::{PreviewEvent, Sequencer, Trigger, samples_per_step};
use synth::{SynthSetup, SynthVoice, VoiceMod};

use crate::instrument::{Instrument, MAX_VOICES, Patch, default_kit, velocity_gain, volume_gain};
use crate::mix::{FxSettings, MasterSettings, level_gain_f, pan_gains};
use crate::model::{
    ChainEntry, ChainId, ChannelStrip, GATE_INF, NoteSet, PhraseId, Step, TICKS_PER_STEP, TRACKS,
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
/// Suavizado de volúmenes, paneo, envíos y retornos.
const GAIN_TAU: f32 = 0.010;
/// Suavizado de los sticks: llegan al valor (o vuelven al guardado) en ~100 ms (spec-ui).
const PERFORM_TAU: f32 = 0.030;
/// Un stick al extremo suma o resta 64 al parámetro 0–127 (spec-ui, "Detalles de los sticks").
const PERFORM_RANGE: f32 = 64.0;
/// Octavas por unidad de corte (0–127 recorre 20 Hz–20 kHz, ~10 octavas).
const CUTOFF_OCTAVES_PER_UNIT: f32 = 9.965_784 / 127.0;
/// Medidores para la pantalla Mezcla: un reporte cada 1600 muestras (30 por segundo).
const METER_FRAMES: usize = 1600;

/// Destinos de los sticks (spec-ui, mapa global): izquierdo X/Y, derecho X/Y.
pub const PERFORM_CUTOFF: usize = 0;
pub const PERFORM_RESONANCE: usize = 1;
pub const PERFORM_DELAY: usize = 2;
pub const PERFORM_REVERB: usize = 3;

/// Mensajes de la interfaz al motor (cola `rtrb`). Todo `Copy`: nada que liberar en el callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    SetInstrument {
        track: u8,
        instrument: Instrument,
    },
    SetTempo(f32),
    /// Swing global en % (50–75; fuera de rango se recorta).
    SetSwing(u8),
    /// Volumen, paneo, envíos, mute y solo de una pista.
    SetMix {
        track: u8,
        strip: ChannelStrip,
    },
    SetFx(FxSettings),
    SetMaster(MasterSettings),
    /// Sticks (performance): desplazamientos en −1..=1 de corte, resonancia, envío a delay y
    /// envío a reverb de una pista. Se suavizan en el motor; 0 vuelve al valor guardado.
    Perform {
        track: u8,
        offsets: [f32; 4],
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
    /// Escuchar una frase entera una vez en una pista (X en la Canción y la Cadena), aunque el
    /// secuenciador esté parado. Play y Stop la cortan.
    PreviewPhrase {
        track: u8,
        phrase: PhraseId,
        transpose: i8,
    },
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
    Meters(Meters),
}

/// Medidores de la pantalla Mezcla, cada `METER_FRAMES` muestras.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Meters {
    /// Pico de cada pista (después del volumen y el paneo) y del master (a la salida).
    pub peaks: [f32; TRACKS + 1],
    /// Máximo de voces de cada pista en la ventana (sin los fundidos).
    pub voices: [u8; TRACKS],
    /// Máximo de voces de la bolsa en la ventana.
    pub pool_voices: u8,
    /// Menor ganancia del limitador en la ventana (1 = no limitó).
    pub limiter_gain: f32,
}

/// Índices de `TrackState::gains`.
const GAIN_L: usize = 0;
const GAIN_R: usize = 1;
const SEND_DELAY_L: usize = 2;
const SEND_DELAY_R: usize = 3;
const SEND_REVERB: usize = 4;

#[derive(Clone, Copy, Debug)]
enum Setup {
    Drum(DrumSetup),
    Synth(SynthSetup),
    Silent,
}

struct TrackState {
    instrument: Instrument,
    setup: Setup,
    strip: ChannelStrip,
    /// Ganancias suavizadas: L, R, envío a delay L y R (después del paneo) y envío a reverb
    /// (mono, después del volumen).
    gains: [Ramp; 5],
    /// Sticks suavizados (índices `PERFORM_*`).
    perform: [Ramp; 4],
    /// Filtro del stick izquierdo para las pistas sin filtro propio.
    bus_filter: BusFilter,
}

impl TrackState {
    fn new(instrument: Instrument, sample_rate: f32) -> Self {
        let mut state = Self {
            instrument,
            setup: prepare(&instrument, sample_rate),
            strip: ChannelStrip::default(),
            gains: [Ramp::new(0.0, GAIN_TAU, sample_rate); 5],
            perform: [Ramp::new(0.0, PERFORM_TAU, sample_rate); 4],
            bus_filter: BusFilter::default(),
        };
        state.retarget();
        for g in &mut state.gains {
            g.settle();
        }
        state
    }

    /// Recalcula los objetivos de las ganancias (al cambiar el instrumento, la mezcla o un
    /// stick). Con la pista centrada y en 100, L = R = el volumen del instrumento, exacto: el
    /// audio sin mezclar es el mismo que antes del H9.
    fn retarget(&mut self) {
        let g = volume_gain(self.instrument.volume) * level_gain_f(f32::from(self.strip.volume));
        let (pl, pr) = pan_gains(self.strip.pan);
        let (l, r) = (g * pl, g * pr);
        let send =
            |base: u8, ramp: &Ramp| level_gain_f(f32::from(base) + PERFORM_RANGE * ramp.value);
        let delay = send(self.strip.delay_send, &self.perform[PERFORM_DELAY]);
        let reverb = send(self.strip.reverb_send, &self.perform[PERFORM_REVERB]);
        let targets = [l, r, l * delay, r * delay, g * reverb];
        for (ramp, t) in self.gains.iter_mut().zip(targets) {
            ramp.target = t;
        }
    }

    fn voice_mod(&self) -> VoiceMod {
        VoiceMod {
            cutoff_octaves: self.perform[PERFORM_CUTOFF].value
                * PERFORM_RANGE
                * CUTOFF_OCTAVES_PER_UNIT,
            resonance: self.perform[PERFORM_RESONANCE].value * PERFORM_RANGE / 127.0,
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
    /// Buses: suma L/R, envíos al delay (L/R) y a la reverb (mono), y salida de un efecto.
    mix: [[f32; BLOCK]; 2],
    delay_bus: [[f32; BLOCK]; 2],
    reverb_bus: [f32; BLOCK],
    fx_out: [[f32; BLOCK]; 2],
    mods: [VoiceMod; TRACKS],
    delay: Delay,
    reverb: Reverb,
    fx: FxSettings,
    /// Retornos de delay y reverb.
    returns: [Ramp; 2],
    master: Ramp,
    limiter: Limiter,
    limiter_on: bool,
    meters: Meters,
    meter_frames: usize,
    voice_counts: [u8; TRACKS],
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
            mix: [[0.0; BLOCK]; 2],
            delay_bus: [[0.0; BLOCK]; 2],
            reverb_bus: [0.0; BLOCK],
            fx_out: [[0.0; BLOCK]; 2],
            mods: [VoiceMod::default(); TRACKS],
            delay: Delay::new(sample_rate),
            reverb: Reverb::new(sample_rate),
            fx: FxSettings::default(),
            returns: [Ramp::new(0.0, GAIN_TAU, sample_rate); 2],
            master: Ramp::new(1.0, GAIN_TAU, sample_rate),
            limiter: Limiter::new(sample_rate),
            limiter_on: MasterSettings::default().limiter,
            meters: Meters {
                limiter_gain: 1.0,
                ..Meters::default()
            },
            meter_frames: 0,
            voice_counts: [0; TRACKS],
            seq: Sequencer::new(samples_per_step(sample_rate, tempo_bpm)),
            frame: 0,
            voices_peak: 0,
            notes_started: 0,
            peak: 0.0,
            #[cfg(test)]
            trigger_log: Vec::with_capacity(4096),
        }
        .with_fx(FxSettings::default())
    }

    fn with_fx(mut self, fx: FxSettings) -> Self {
        self.set_fx(fx);
        for r in &mut self.returns {
            r.settle();
        }
        self
    }

    fn set_fx(&mut self, fx: FxSettings) {
        self.fx = fx;
        self.delay.set(&fx.delay, self.tempo_bpm, self.sample_rate);
        self.reverb.set(&fx.reverb, self.sample_rate);
        self.returns[0].target = level_gain_f(f32::from(fx.delay.return_level));
        self.returns[1].target = level_gain_f(f32::from(fx.reverb.return_level));
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
                state.retarget();
            }
            Command::SetTempo(bpm) => {
                if bpm.is_finite() {
                    self.tempo_bpm = bpm.clamp(20.0, 999.0);
                    let sps = samples_per_step(self.sample_rate, self.tempo_bpm);
                    self.seq.set_samples_per_step(sps, self.frame);
                    // El delay sigue al tempo (se desliza al tiempo nuevo).
                    self.delay
                        .set(&self.fx.delay, self.tempo_bpm, self.sample_rate);
                }
            }
            Command::SetSwing(swing) => self.seq.set_swing(swing),
            Command::SetMix { track, strip } => {
                if let Some(state) = self.tracks.get_mut(usize::from(track)) {
                    let audible_changed =
                        (state.strip.mute, state.strip.solo) != (strip.mute, strip.solo);
                    state.strip = strip;
                    state.retarget();
                    if audible_changed {
                        self.update_audible();
                    }
                }
            }
            Command::SetFx(fx) => self.set_fx(fx),
            Command::SetMaster(m) => {
                self.master.target = m.gain_linear();
                self.limiter_on = m.limiter;
            }
            Command::Perform { track, offsets } => {
                if let Some(state) = self.tracks.get_mut(usize::from(track)) {
                    for (ramp, o) in state.perform.iter_mut().zip(offsets) {
                        ramp.target = if o.is_finite() {
                            o.clamp(-1.0, 1.0)
                        } else {
                            0.0
                        };
                    }
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
                self.cancel_preview();
                if self.seq.is_playing() {
                    self.release_all();
                }
                self.seq.play(mode, self.frame);
            }
            Command::Stop => {
                self.cancel_preview();
                if self.seq.is_playing() {
                    self.seq.stop();
                    self.release_all();
                }
            }
            Command::PreviewPhrase {
                track,
                phrase,
                transpose,
            } => {
                if usize::from(track) < TRACKS {
                    self.cancel_preview();
                    self.seq.start_preview(track, phrase, transpose, self.frame);
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
        let any_solo = self.tracks.iter().any(|t| t.strip.solo);
        let mut audible = [true; TRACKS];
        for (t, (a, state)) in audible.iter_mut().zip(&self.tracks).enumerate() {
            *a = !state.strip.mute && (!any_solo || state.strip.solo);
            // Lo que queda sostenido en una pista que se apaga se suelta (sigue su relajación).
            if !*a {
                self.pool.release_track(t as u8, false);
            }
        }
        self.seq.set_audible(audible);
    }

    /// Corta la vista previa: lo que tenía INF en su pista se suelta.
    fn cancel_preview(&mut self) {
        if let Some(track) = self.seq.cancel_preview() {
            self.pool.release_track(track, true);
        }
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
            if let Some(trigger) = trigger {
                self.apply(t, trigger);
            }
        }
    }

    /// Dispara el paso de la vista previa que cae en la muestra actual (o la termina).
    fn fire_preview(&mut self) {
        match self.seq.fire_preview() {
            Some(PreviewEvent::Step {
                track,
                trigger: Some(trigger),
            }) => self.apply(usize::from(track), trigger),
            Some(PreviewEvent::End { track }) => self.pool.release_track(track, true),
            _ => {}
        }
    }

    fn apply(&mut self, t: usize, trigger: Trigger) {
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
                let gate =
                    (gate_ticks != GATE_INF).then(|| self.ticks_to_samples(u32::from(gate_ticks)));
                self.note_on(t, notes.as_slice(), velocity, gate);
            }
        }
    }

    /// Dispara lo que cae en la muestra actual (pasos del secuenciador y de la vista previa) y
    /// devuelve la muestra del próximo evento.
    fn fire_due_events(&mut self) -> Option<u64> {
        loop {
            let step = self.seq.next_step_frame();
            let preview = self.seq.next_preview_frame();
            if step.is_some_and(|f| f <= self.frame) {
                self.fire_step();
            } else if preview.is_some_and(|f| f <= self.frame) {
                self.fire_preview();
            } else {
                return match (step, preview) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
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

    /// Medidores acumulados, cada `METER_FRAMES` muestras (para el evento `Meters`).
    pub fn take_meters(&mut self) -> Option<Meters> {
        if self.meter_frames < METER_FRAMES {
            return None;
        }
        // Se descuenta (no se pone en 0): un reporte cada METER_FRAMES aunque no sea múltiplo
        // del buffer.
        self.meter_frames = (self.meter_frames - METER_FRAMES).min(METER_FRAMES);
        let m = self.meters;
        self.meters = Meters {
            limiter_gain: 1.0,
            ..Meters::default()
        };
        Some(m)
    }

    /// No hay nada que renderizar: ni voces ni colas de efectos.
    fn is_quiet(&self) -> bool {
        self.pool.in_use() == 0 && !self.delay.is_active() && !self.reverb.is_active()
    }

    /// Llena `out` (intercalado, `channels` canales): L y R en los dos primeros (en mono, el
    /// promedio) y silencio en los demás. Los pasos del secuenciador se disparan en su muestra
    /// exacta dentro del buffer.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let mut pos = 0;
        while pos < frames {
            let next_event = self.fire_due_events();
            let mut n = (BLOCK - (self.frame % BLOCK as u64) as usize).min(frames - pos);
            if let Some(due) = next_event {
                n = n.min((due - self.frame) as usize);
            }
            let chunk = &mut out[pos * channels..(pos + n) * channels];
            let voices = self.pool.in_use();
            self.voices_peak = self.voices_peak.max(voices);
            self.count_voices();
            if self.is_quiet() {
                chunk.fill(0.0);
                self.settle_all();
            } else {
                self.render_block(n);
                let [l, r] = &self.mix;
                for (frame, (&l, &r)) in chunk.chunks_exact_mut(channels).zip(l.iter().zip(r)) {
                    match frame {
                        [mono] => *mono = 0.5 * (l + r),
                        [a, b, rest @ ..] => {
                            *a = l;
                            *b = r;
                            rest.fill(0.0);
                        }
                        [] => {}
                    }
                }
            }
            pos += n;
            self.frame += n as u64;
            self.meter_frames += n;
        }
        out[frames * channels..].fill(0.0);
    }

    fn count_voices(&mut self) {
        self.pool.count_by_track(&mut self.voice_counts);
        let m = &mut self.meters;
        let mut total = 0;
        for (max, &c) in m.voices.iter_mut().zip(&self.voice_counts) {
            *max = (*max).max(c);
            total += c;
        }
        m.pool_voices = m.pool_voices.max(total);
    }

    /// Sin señal, los parámetros saltan a su valor (no hay nada que proteger de clicks).
    fn settle_all(&mut self) {
        self.master.settle();
        for r in &mut self.returns {
            r.settle();
        }
        for t in &mut self.tracks {
            // Un stick movido en silencio cambia los envíos: se recalculan (con `powf`) solo
            // entonces; si no, el motor parado no gasta nada acá.
            if !t.perform.iter().all(Ramp::is_settled) {
                for p in &mut t.perform {
                    p.settle();
                }
                t.retarget();
            }
            for g in &mut t.gains {
                g.settle();
            }
        }
        self.limiter.reset();
    }

    /// ~4 operaciones por muestra y pista sin envíos, ~8 con los dos; los efectos y el master
    /// aparte (ver dsp.md, "Costo").
    fn render_block(&mut self, n: usize) {
        for (m, t) in self.mods.iter_mut().zip(&mut self.tracks) {
            if !t.perform.iter().all(Ramp::is_settled) {
                for p in &mut t.perform {
                    p.advance(n);
                }
                t.retarget();
            }
            *m = t.voice_mod();
        }
        let mask = self.pool.render(&mut self.bufs, n, &self.mods);
        for bus in self.mix.iter_mut().chain(&mut self.delay_bus) {
            bus[..n].fill(0.0);
        }
        self.reverb_bus[..n].fill(0.0);
        let (mut to_delay, mut to_reverb) = (false, false);
        let sr = self.sample_rate;
        for (t, (state, buf)) in self.tracks.iter_mut().zip(&mut self.bufs).enumerate() {
            if mask & (1 << t) == 0 {
                for g in &mut state.gains {
                    g.settle();
                }
                continue;
            }
            let buf = &mut buf[..n];
            if !matches!(state.setup, Setup::Synth(_)) {
                let p = &state.perform;
                state.bus_filter.process(
                    buf,
                    p[PERFORM_CUTOFF].value,
                    p[PERFORM_RESONANCE].value,
                    sr,
                );
            }
            let g = &mut state.gains;
            let level = buf.iter().fold(0.0f32, |m, x| m.max(x.abs()))
                * g[GAIN_L].value.max(g[GAIN_R].value);
            self.meters.peaks[t] = self.meters.peaks[t].max(level);
            let [mix_l, mix_r] = &mut self.mix;
            mix_into(&mut mix_l[..n], buf, &mut g[GAIN_L]);
            mix_into(&mut mix_r[..n], buf, &mut g[GAIN_R]);
            to_delay |= !g[SEND_DELAY_L].is_zero() || !g[SEND_DELAY_R].is_zero();
            to_reverb |= !g[SEND_REVERB].is_zero();
            let [delay_l, delay_r] = &mut self.delay_bus;
            mix_into(&mut delay_l[..n], buf, &mut g[SEND_DELAY_L]);
            mix_into(&mut delay_r[..n], buf, &mut g[SEND_DELAY_R]);
            mix_into(&mut self.reverb_bus[..n], buf, &mut g[SEND_REVERB]);
            buf.fill(0.0);
        }
        self.render_fx(n, to_delay, to_reverb);
        self.render_master(n);
    }

    fn render_fx(&mut self, n: usize, to_delay: bool, to_reverb: bool) {
        let [out_l, out_r] = &mut self.fx_out;
        let [mix_l, mix_r] = &mut self.mix;
        if to_delay || self.delay.is_active() {
            let [in_l, in_r] = &self.delay_bus;
            self.delay
                .process(&in_l[..n], &in_r[..n], &mut out_l[..n], &mut out_r[..n]);
            let mut ret = self.returns[0];
            mix_into(&mut mix_l[..n], &out_l[..n], &mut ret);
            mix_into(&mut mix_r[..n], &out_r[..n], &mut self.returns[0]);
        } else {
            self.returns[0].settle();
        }
        if to_reverb || self.reverb.is_active() {
            self.reverb
                .process(&self.reverb_bus[..n], &mut out_l[..n], &mut out_r[..n]);
            let mut ret = self.returns[1];
            mix_into(&mut mix_l[..n], &out_l[..n], &mut ret);
            mix_into(&mut mix_r[..n], &out_r[..n], &mut self.returns[1]);
        } else {
            self.returns[1].settle();
        }
    }

    /// Ganancia master → limitador (si está activo) → soft clip. ~12 operaciones por frame.
    fn render_master(&mut self, n: usize) {
        let [l, r] = &mut self.mix;
        let (l, r) = (&mut l[..n], &mut r[..n]);
        let (mut g, inc) = self.master.advance(n);
        if inc == 0.0 {
            if g != 1.0 {
                for (x, y) in l.iter_mut().zip(r.iter_mut()) {
                    *x *= g;
                    *y *= g;
                }
            }
        } else {
            for (x, y) in l.iter_mut().zip(r.iter_mut()) {
                g += inc;
                *x *= g;
                *y *= g;
            }
        }
        if self.limiter_on {
            self.limiter.process(l, r);
            self.meters.limiter_gain = self.meters.limiter_gain.min(self.limiter.min_gain);
            self.limiter.min_gain = 1.0;
        }
        let mut peak = self.peak;
        let mut master_peak = self.meters.peaks[TRACKS];
        for (x, y) in l.iter_mut().zip(r.iter_mut()) {
            *x = soft_clip(*x);
            *y = soft_clip(*y);
            let p = x.abs().max(y.abs());
            peak = peak.max(p);
            master_peak = master_peak.max(p);
        }
        self.peak = peak;
        self.meters.peaks[TRACKS] = master_peak;
    }
}

#[cfg(test)]
mod seq_tests;
#[cfg(test)]
mod tests;
