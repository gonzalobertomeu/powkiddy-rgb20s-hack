//! Secuenciador (H3 y H5): reloj en muestras, un cabezal por pista, modos de reproducción y la
//! copia de frases, cadenas y canción que usa el hilo de audio (spec-motor, "Secuenciador").
//!
//! Edición en vivo: la interfaz es dueña del `Project` y manda cada cambio como un comando
//! chico (`SetStep`, `SetChainEntry`, `SetSongCell`) que el motor aplica a `SongData`, que se
//! asigna una sola vez al crear el motor. Un paso se lee recién cuando el cabezal lo dispara: lo
//! que se edita adelante del cabezal suena en esta vuelta; lo de atrás, en la siguiente.

use crate::model::{
    CHAIN_LEN, ChainEntry, ChainId, Chord, MAX_CHAINS, MAX_PHRASES, MAX_SONG_ROWS, NOTE_MAX,
    NOTE_MIN, NoteCell, PHRASE_STEPS, PhraseId, SWING_MAX, SWING_MIN, Step, TRACKS,
};

/// Qué suena. Los modos de una pista guardan la posición `(fila, entrada)` y no el número de
/// cadena o de frase: el cabezal resuelve el número en cada paso, igual que en la canción, así
/// cambiar la celda o la FRA mientras suena se oye en el paso siguiente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    /// Las 8 pistas desde la fila `from_row`, cada una por su columna con la regla de bloques.
    Song { from_row: u8 },
    /// Las 8 pistas en loop sobre la sección `(row, entry)`: cada una repite su frase en la
    /// entrada `entry` de su cadena en la fila `row`.
    Section { row: u8, entry: u8 },
    /// Una pista en loop sobre la cadena de la celda `(row, track)`.
    Chain { track: u8, row: u8 },
    /// Una pista en loop sobre la frase de la entrada `entry` de su cadena en la fila `row`.
    Phrase { track: u8, row: u8, entry: u8 },
}

/// Posición de un cabezal. `active = false`: la pista no suena en este modo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeadPos {
    pub row: u8,
    pub entry: u8,
    pub step: u8,
    pub active: bool,
}

/// Lo que el motor le publica a la interfaz para dibujar los cabezales.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Playhead {
    pub playing: bool,
    pub heads: [HeadPos; TRACKS],
}

/// Un disparo del paso actual de una pista. El motor decide las notas (la percusión toca solo
/// la raíz, el resto el acorde).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Notes {
        root: u8,
        chord: Chord,
        velocity: u8,
        gate_ticks: u16,
    },
    /// `OFF`: libera todas las voces de la pista.
    Off,
}

/// Copia de la estructura de la canción en el hilo de audio. Las frases que no existen en el
/// proyecto se ven como frases vacías.
pub struct SongData {
    phrases: Box<[[Step; PHRASE_STEPS]]>,
    chains: Box<[[Option<ChainEntry>; CHAIN_LEN]]>,
    rows: Box<[[Option<ChainId>; TRACKS]]>,
}

impl SongData {
    pub fn new() -> Self {
        Self {
            phrases: vec![[Step::default(); PHRASE_STEPS]; MAX_PHRASES].into_boxed_slice(),
            chains: vec![[None; CHAIN_LEN]; MAX_CHAINS].into_boxed_slice(),
            rows: vec![[None; TRACKS]; MAX_SONG_ROWS].into_boxed_slice(),
        }
    }

    pub fn set_step(&mut self, phrase: PhraseId, step: usize, value: Step) {
        if let Some(s) = self.phrases[usize::from(phrase)].get_mut(step) {
            *s = value;
        }
    }

    pub fn set_chain_entry(&mut self, chain: ChainId, pos: usize, value: Option<ChainEntry>) {
        if let Some(e) = self.chains[usize::from(chain)].get_mut(pos) {
            *e = value;
        }
    }

    pub fn set_song_cell(&mut self, row: usize, track: usize, chain: Option<ChainId>) {
        if let Some(cell) = self.rows.get_mut(row).and_then(|r| r.get_mut(track)) {
            *cell = chain;
        }
    }

    pub fn cell(&self, row: usize, track: usize) -> Option<ChainId> {
        *self.rows.get(row)?.get(track)?
    }

    pub fn entry_at(&self, track: usize, row: usize, entry: usize) -> Option<ChainEntry> {
        let chain = self.cell(row, track)?;
        *self.chains[usize::from(chain)].get(entry)?
    }

    /// Primera fila del bloque de `row` en la columna de `track`: sube mientras la fila de
    /// arriba tenga cadena (como mucho 255 lecturas, solo al terminar una cadena).
    pub fn block_start(&self, track: usize, row: usize) -> usize {
        let mut start = row.min(MAX_SONG_ROWS - 1);
        while start > 0 && self.cell(start - 1, track).is_some() {
            start -= 1;
        }
        start
    }

    /// Adónde va un cabezal al terminar la frase de `(row, entry)` (spec-motor, "Cabezales
    /// por pista"): la entrada siguiente de la cadena; si no hay, la fila siguiente de su
    /// columna; si esa celda está vacía, la primera fila de su bloque.
    pub fn next_position(&self, mode: PlayMode, track: usize, row: u8, entry: u8) -> (u8, u8) {
        let (r, e) = (usize::from(row), usize::from(entry));
        match mode {
            PlayMode::Section { .. } | PlayMode::Phrase { .. } => (row, entry),
            PlayMode::Chain { .. } => {
                if self.entry_at(track, r, e + 1).is_some() {
                    (row, entry + 1)
                } else {
                    (row, 0)
                }
            }
            PlayMode::Song { .. } => {
                if self.entry_at(track, r, e + 1).is_some() {
                    (row, entry + 1)
                } else if r + 1 < MAX_SONG_ROWS && self.cell(r + 1, track).is_some() {
                    (row + 1, 0)
                } else {
                    (self.block_start(track, r) as u8, 0)
                }
            }
        }
    }

    pub fn step(&self, phrase: PhraseId, step: usize) -> &Step {
        &self.phrases[usize::from(phrase)][step]
    }
}

impl Default for SongData {
    fn default() -> Self {
        Self::new()
    }
}

/// Reloj de pasos. El paso `n` desde `origin` cae en `origin + n · samples_per_step` (la grilla
/// recta) más el atraso del swing si es el segundo de su par: la posición se calcula, no se
/// acumula, así no hay deriva por redondeo en sesiones largas.
#[derive(Clone, Copy, Debug)]
struct Clock {
    samples_per_step: f64,
    origin: f64,
    /// Pasos desde `origin` (vuelve a 0 con cada cambio de tempo).
    count: u64,
    /// Pasos desde Play: decide la paridad del swing (no cambia con el tempo).
    step: u64,
}

impl Clock {
    fn new(samples_per_step: f64) -> Self {
        Self {
            samples_per_step,
            origin: 0.0,
            count: 0,
            step: 0,
        }
    }

    fn start(&mut self, frame: u64) {
        self.origin = frame as f64;
        self.count = 0;
        self.step = 0;
    }

    /// Dónde cae el próximo paso sin swing.
    fn grid(&self) -> f64 {
        self.origin + self.count as f64 * self.samples_per_step
    }

    /// Dónde cae el próximo paso, en muestras (con decimales).
    fn next(&self, swing: u8) -> f64 {
        let delay = if self.step % 2 == 1 {
            swing_fraction(swing) * self.samples_per_step
        } else {
            0.0
        };
        self.grid() + delay
    }

    fn tick(&mut self) {
        self.count += 1;
        self.step += 1;
    }

    /// Nuevo tempo con la misma fase respecto de la grilla. Si la grilla ya pasó y el paso
    /// espera su swing, la fracción ya esperada también se conserva.
    fn retime(&mut self, samples_per_step: f64, frame: u64) {
        let now = frame as f64;
        let remaining = ((self.grid() - now) / self.samples_per_step).clamp(-1.0, 1.0);
        self.origin = now + remaining * samples_per_step;
        self.count = 0;
        self.samples_per_step = samples_per_step;
    }
}

/// Atraso de los pasos pares de cada par, en pasos (spec-motor, "Swing"): 0 a 0,5.
pub fn swing_fraction(swing: u8) -> f64 {
    f64::from(swing.clamp(SWING_MIN, SWING_MAX) - SWING_MIN) / 50.0
}

/// Vista previa de una frase (X en la Canción y la Cadena): sus 16 pasos una vez, en una pista,
/// con su propio reloj (suena aunque el secuenciador esté parado). Comparte tempo y swing.
#[derive(Clone, Copy, Debug)]
struct Preview {
    track: u8,
    phrase: PhraseId,
    transpose: i8,
    clock: Clock,
}

/// Lo que pasa en la muestra de la vista previa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewEvent {
    Step {
        track: u8,
        trigger: Option<Trigger>,
    },
    /// Terminó la frase: se sueltan las notas INF de la pista.
    End {
        track: u8,
    },
}

pub struct Sequencer {
    pub data: SongData,
    mode: Option<PlayMode>,
    heads: [HeadPos; TRACKS],
    clock: Clock,
    swing: u8,
    /// Ya se disparó el primer paso desde Play (antes de eso no se avanza).
    started: bool,
    audible: [bool; TRACKS],
    playhead_dirty: bool,
    preview: Option<Preview>,
}

/// Qué dispara un paso, con la transposición de su entrada de cadena.
fn trigger_of(step: &Step, transpose: i8) -> Option<Trigger> {
    match step.note {
        NoteCell::Empty => None,
        NoteCell::Off => Some(Trigger::Off),
        NoteCell::On(n) => Some(Trigger::Notes {
            root: (i16::from(n) + i16::from(transpose))
                .clamp(i16::from(NOTE_MIN), i16::from(NOTE_MAX)) as u8,
            chord: step.chord,
            velocity: step.velocity,
            gate_ticks: step.gate_ticks,
        }),
    }
}

pub fn samples_per_step(sample_rate: f32, bpm: f32) -> f64 {
    f64::from(sample_rate) * 60.0 / (f64::from(bpm) * 4.0)
}

impl Sequencer {
    pub fn new(samples_per_step: f64) -> Self {
        Self {
            data: SongData::new(),
            mode: None,
            heads: [HeadPos::default(); TRACKS],
            clock: Clock::new(samples_per_step),
            swing: SWING_MIN,
            started: false,
            audible: [true; TRACKS],
            playhead_dirty: false,
            preview: None,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.mode.is_some()
    }

    /// Arranca (o reinicia) en `frame`: el primer paso suena en esa misma muestra.
    pub fn play(&mut self, mode: PlayMode, frame: u64) {
        for (t, h) in self.heads.iter_mut().enumerate() {
            let (row, entry, active) = match mode {
                // Una pista cuya celda de inicio está vacía no suena en toda la canción.
                PlayMode::Song { from_row } => {
                    (from_row, 0, self.data.cell(from_row.into(), t).is_some())
                }
                PlayMode::Section { row, entry } => (row, entry, true),
                PlayMode::Chain { track, row } => (row, 0, usize::from(track) == t),
                PlayMode::Phrase { track, row, entry } => (row, entry, usize::from(track) == t),
            };
            *h = HeadPos {
                row,
                entry,
                step: 0,
                active,
            };
        }
        self.mode = Some(mode);
        self.clock.start(frame);
        self.started = false;
        self.playhead_dirty = true;
    }

    pub fn stop(&mut self) {
        self.mode = None;
        for h in &mut self.heads {
            h.active = false;
        }
        self.playhead_dirty = true;
    }

    /// Cambia el tempo conservando la fase del paso en curso: el paso siguiente llega cuando le
    /// falta la misma fracción de paso que antes, al tempo nuevo.
    pub fn set_samples_per_step(&mut self, samples_per_step: f64, frame: u64) {
        if self.is_playing() && self.started {
            self.clock.retime(samples_per_step, frame);
        } else {
            self.clock.samples_per_step = samples_per_step;
        }
        if let Some(p) = &mut self.preview {
            if p.clock.step > 0 {
                p.clock.retime(samples_per_step, frame);
            } else {
                p.clock.samples_per_step = samples_per_step;
            }
        }
    }

    /// Empieza (o reemplaza) la vista previa: el primer paso suena en `frame`.
    pub fn start_preview(&mut self, track: u8, phrase: PhraseId, transpose: i8, frame: u64) {
        let mut clock = Clock::new(self.clock.samples_per_step);
        clock.start(frame);
        self.preview = Some(Preview {
            track,
            phrase,
            transpose,
            clock,
        });
    }

    /// Corta la vista previa; devuelve su pista.
    pub fn cancel_preview(&mut self) -> Option<u8> {
        self.preview.take().map(|p| p.track)
    }

    /// Muestra del próximo evento de la vista previa, si hay una.
    pub fn next_preview_frame(&self) -> Option<u64> {
        self.preview
            .map(|p| p.clock.next(self.swing).round().max(0.0) as u64)
    }

    /// El paso de la vista previa que toca (o su final, después del paso 16).
    pub fn fire_preview(&mut self) -> Option<PreviewEvent> {
        let p = self.preview.as_mut()?;
        let track = p.track;
        let i = p.clock.step as usize;
        if i >= PHRASE_STEPS {
            self.preview = None;
            return Some(PreviewEvent::End { track });
        }
        p.clock.tick();
        let step = self.data.step(p.phrase, i);
        let trigger = trigger_of(step, p.transpose);
        Some(PreviewEvent::Step { track, trigger })
    }

    /// Swing en % (50 = recto, 75 = medio paso). Se aplica desde el próximo paso: si el paso
    /// que espera ya pasó su momento con el swing nuevo, suena en la muestra siguiente.
    pub fn set_swing(&mut self, swing: u8) {
        self.swing = swing.clamp(SWING_MIN, SWING_MAX);
    }

    pub fn set_audible(&mut self, audible: [bool; TRACKS]) {
        self.audible = audible;
    }

    /// Muestra (absoluta) del próximo paso, si está sonando.
    pub fn next_step_frame(&self) -> Option<u64> {
        self.is_playing()
            .then(|| self.clock.next(self.swing).round().max(0.0) as u64)
    }

    /// Avanza los cabezales al paso que toca y devuelve qué disparar en cada pista.
    pub fn fire(&mut self) -> [Option<Trigger>; TRACKS] {
        let mut out = [None; TRACKS];
        if !self.is_playing() {
            return out;
        }
        if self.started {
            for t in 0..TRACKS {
                self.advance(t);
            }
        }
        self.started = true;
        self.clock.tick();
        self.playhead_dirty = true;

        for (t, slot) in out.iter_mut().enumerate() {
            let h = self.heads[t];
            if !h.active || !self.audible[t] {
                continue;
            }
            let Some(entry) = self
                .data
                .entry_at(t, usize::from(h.row), usize::from(h.entry))
            else {
                continue;
            };
            let step = self.data.step(entry.phrase, usize::from(h.step));
            *slot = trigger_of(step, entry.transpose);
        }
        out
    }

    fn advance(&mut self, track: usize) {
        let Some(mode) = self.mode else { return };
        let h = &mut self.heads[track];
        if !h.active {
            return;
        }
        h.step += 1;
        if usize::from(h.step) == PHRASE_STEPS {
            h.step = 0;
            (h.row, h.entry) = self.data.next_position(mode, track, h.row, h.entry);
        }
    }

    /// La posición, si cambió desde la última vez que se tomó.
    pub fn take_playhead(&mut self) -> Option<Playhead> {
        if !self.playhead_dirty {
            return None;
        }
        self.playhead_dirty = false;
        Some(Playhead {
            playing: self.is_playing(),
            heads: self.heads,
        })
    }
}
