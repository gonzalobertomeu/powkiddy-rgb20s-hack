//! Modelo de datos del proyecto: canción → cadenas → frases (spec-motor, "Modelo de datos").
//!
//! Sin I/O ni audio. Las referencias entre canción, cadenas y frases son IDs `u8`, así varias
//! cadenas comparten una frase y el formato de archivo (H6) es directo. Los campos de v1
//! (locks, condiciones, micro, repeticiones) se agregan con el formato de archivo.

use std::fmt;

use crate::instrument::{Instrument, default_kit};

pub const TRACKS: usize = 8;
pub const PHRASE_STEPS: usize = 16;
pub const CHAIN_LEN: usize = 16;
pub const MAX_PHRASES: usize = 256;
pub const MAX_CHAINS: usize = 256;
pub const MAX_SONG_ROWS: usize = 256;
pub const TICKS_PER_STEP: u16 = 24;
/// Transposición de una entrada de cadena, en semitonos (±4 octavas).
pub const TRANSPOSE_MAX: i8 = 48;

pub const NOTE_MIN: u8 = 12; // C-0
pub const NOTE_MAX: u8 = 127; // G-9
pub const DEFAULT_VELOCITY: u8 = 100;
pub const DEFAULT_GATE: u16 = TICKS_PER_STEP;
/// Largo "hasta la próxima nota de la pista".
pub const GATE_INF: u16 = u16::MAX;
/// Valores de LARGO en ticks: 1/4, 1/2, 3/4, 1 a 16 pasos e INF.
pub const GATES: [u16; 20] = [
    6, 12, 18, 24, 48, 72, 96, 120, 144, 168, 192, 216, 240, 264, 288, 312, 336, 360, 384, GATE_INF,
];

pub type PhraseId = u8;
pub type ChainId = u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NoteCell {
    #[default]
    Empty,
    On(u8),
    /// Libera todas las voces de la pista.
    #[expect(dead_code, reason = "la spec todavía no define cómo se ingresa un OFF")]
    Off,
}

/// Nombre estilo tracker: `C-4`, `C#4`. C-0 = MIDI 12, así C-4 = 60.
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
    ];
    let octave = i32::from(note) / 12 - 1;
    format!("{}{}", NAMES[usize::from(note % 12)], octave)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ChordKind {
    #[default]
    None,
    Fifth,
    Octave,
    Major,
    Minor,
    Dim,
    Aug,
    Sus2,
    Sus4,
    Maj7,
    Min7,
    Dom7,
    Min7b5,
    Add9,
}

impl ChordKind {
    pub const ALL: [ChordKind; 14] = [
        ChordKind::None,
        ChordKind::Fifth,
        ChordKind::Octave,
        ChordKind::Major,
        ChordKind::Minor,
        ChordKind::Dim,
        ChordKind::Aug,
        ChordKind::Sus2,
        ChordKind::Sus4,
        ChordKind::Maj7,
        ChordKind::Min7,
        ChordKind::Dom7,
        ChordKind::Min7b5,
        ChordKind::Add9,
    ];

    /// Semitonos sobre la raíz, incluida la raíz.
    pub fn intervals(self) -> &'static [u8] {
        match self {
            ChordKind::None => &[0],
            ChordKind::Fifth => &[0, 7],
            ChordKind::Octave => &[0, 12],
            ChordKind::Major => &[0, 4, 7],
            ChordKind::Minor => &[0, 3, 7],
            ChordKind::Dim => &[0, 3, 6],
            ChordKind::Aug => &[0, 4, 8],
            ChordKind::Sus2 => &[0, 2, 7],
            ChordKind::Sus4 => &[0, 5, 7],
            ChordKind::Maj7 => &[0, 4, 7, 11],
            ChordKind::Min7 => &[0, 3, 7, 10],
            ChordKind::Dom7 => &[0, 4, 7, 10],
            ChordKind::Min7b5 => &[0, 3, 6, 10],
            ChordKind::Add9 => &[0, 4, 7, 14],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ChordKind::None => "---",
            ChordKind::Fifth => "5",
            ChordKind::Octave => "8va",
            ChordKind::Major => "may",
            ChordKind::Minor => "men",
            ChordKind::Dim => "dis",
            ChordKind::Aug => "aum",
            ChordKind::Sus2 => "sus2",
            ChordKind::Sus4 => "sus4",
            ChordKind::Maj7 => "may7",
            ChordKind::Min7 => "men7",
            ChordKind::Dom7 => "dom7",
            ChordKind::Min7b5 => "m7b5",
            ChordKind::Add9 => "add9",
        }
    }

    /// Recorre la lista en forma circular.
    pub fn cycle(self, delta: i32) -> ChordKind {
        let n = Self::ALL.len() as i32;
        let i = Self::ALL.iter().position(|&k| k == self).unwrap_or(0) as i32;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

/// Hasta 4 notas, sin memoria dinámica (se manda tal cual al motor).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct NoteSet {
    notes: [u8; 4],
    len: u8,
}

impl NoteSet {
    pub fn single(note: u8) -> Self {
        Self {
            notes: [note, 0, 0, 0],
            len: 1,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.notes[..usize::from(self.len)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Chord {
    pub kind: ChordKind,
    pub inversion: u8,
}

impl Chord {
    pub fn max_inversion(self) -> u8 {
        self.kind.intervals().len() as u8 - 1
    }

    pub fn label(self) -> String {
        match (self.kind, self.inversion) {
            (ChordKind::None, _) => "---".to_string(),
            (kind, 0) => kind.label().to_string(),
            (kind, inv) => format!("{}/{inv}", kind.label()),
        }
    }

    /// Notas del acorde sobre `root`, de grave a agudo. Cada inversión sube una octava la nota
    /// más grave; las que pasan de G-9 se descartan.
    pub fn notes(self, root: u8) -> NoteSet {
        let mut notes = [0u16; 4];
        let intervals = self.kind.intervals();
        for (n, &i) in notes.iter_mut().zip(intervals) {
            *n = u16::from(root) + u16::from(i);
        }
        let len = intervals.len();
        for _ in 0..self.inversion.min(self.max_inversion()) {
            notes[..len].sort_unstable();
            notes[0] += 12;
        }
        notes[..len].sort_unstable();
        let mut set = NoteSet::default();
        for &n in &notes[..len] {
            if n <= u16::from(NOTE_MAX) {
                set.notes[usize::from(set.len)] = n as u8;
                set.len += 1;
            }
        }
        set
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub note: NoteCell,
    pub chord: Chord,
    pub velocity: u8,
    pub gate_ticks: u16,
}

impl Default for Step {
    fn default() -> Self {
        Self {
            note: NoteCell::Empty,
            chord: Chord::default(),
            velocity: DEFAULT_VELOCITY,
            gate_ticks: DEFAULT_GATE,
        }
    }
}

impl Step {
    pub fn is_empty(&self) -> bool {
        self.note == NoteCell::Empty
    }

    pub fn root(&self) -> Option<u8> {
        match self.note {
            NoteCell::On(n) => Some(n),
            _ => None,
        }
    }
}

/// Texto de la columna LARGO.
pub fn gate_label(ticks: u16) -> String {
    match ticks {
        GATE_INF => "INF".to_string(),
        6 => "1/4".to_string(),
        12 => "1/2".to_string(),
        18 => "3/4".to_string(),
        t => format!("{}", t / TICKS_PER_STEP),
    }
}

/// Mueve `ticks` `delta` posiciones dentro de `GATES` (sin dar la vuelta).
pub fn gate_nudge(ticks: u16, delta: i32) -> u16 {
    let i = GATES
        .iter()
        .position(|&g| g >= ticks)
        .unwrap_or(GATES.len() - 1) as i32;
    GATES[(i + delta).clamp(0, GATES.len() as i32 - 1) as usize]
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Phrase {
    pub steps: [Step; PHRASE_STEPS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainEntry {
    pub phrase: PhraseId,
    /// Semitonos, -48..=48.
    pub transpose: i8,
}

/// Lista de frases. Las entradas son contiguas: la primera `None` termina la cadena.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Chain {
    pub entries: [Option<ChainEntry>; CHAIN_LEN],
}

impl Chain {
    pub fn len(&self) -> usize {
        self.entries.iter().take_while(|e| e.is_some()).count()
    }

    pub fn entry(&self, k: usize) -> Option<ChainEntry> {
        self.entries.get(k).copied().flatten()
    }

    /// Quita la entrada `k`; las siguientes suben.
    fn remove(&mut self, k: usize) -> bool {
        if k >= self.len() {
            return false;
        }
        self.entries[k..].rotate_left(1);
        self.entries[CHAIN_LEN - 1] = None;
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    Drum,
    Sample,
    Synth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelStrip {
    pub volume: u8,
    pub pan: i8,
    pub delay_send: u8,
    pub reverb_send: u8,
    pub mute: bool,
    pub solo: bool,
}

impl Default for ChannelStrip {
    fn default() -> Self {
        Self {
            volume: 100,
            pan: 0,
            delay_send: 0,
            reverb_send: 0,
            mute: false,
            solo: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub name: String,
    pub instrument: Instrument,
    pub mix: ChannelStrip,
}

impl Track {
    pub fn kind(&self) -> TrackKind {
        self.instrument.kind()
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Song {
    pub rows: Vec<[Option<ChainId>; TRACKS]>,
}

/// Posición `(R, k)`: fila de la canción y entrada de cadena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Section {
    pub row: usize,
    pub entry: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditError {
    NoFreeChain,
    NoFreePhrase,
    SongFull,
    ChainFull,
    /// La cadena de la pista termina antes de la entrada pedida (largo actual).
    Gap {
        chain_len: usize,
    },
    NoPhrase,
    NoChain,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::NoFreeChain => write!(f, "no quedan cadenas libres"),
            EditError::NoFreePhrase => write!(f, "no quedan frases libres"),
            EditError::SongFull => write!(f, "la canción ya tiene {MAX_SONG_ROWS} filas"),
            EditError::ChainFull => write!(f, "la cadena ya tiene {CHAIN_LEN} frases"),
            EditError::Gap { chain_len } => {
                write!(
                    f,
                    "la cadena de esta pista termina en la sección {chain_len}"
                )
            }
            EditError::NoPhrase => write!(f, "no hay frase en esta sección"),
            EditError::NoChain => write!(f, "no hay cadena en esta celda"),
        }
    }
}

/// Qué se creó al asegurar una frase en una sección.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ensured {
    pub phrase: PhraseId,
    pub new_chain: Option<ChainId>,
    /// La frase se agregó a la cadena en esta llamada.
    pub added_entry: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub name: String,
    pub tempo_bpm: f32,
    pub swing: u8,
    pub tracks: [Track; TRACKS],
    pub song: Song,
    pub chains: Box<[Option<Chain>]>,
    pub phrases: Box<[Option<Phrase>]>,
}

impl Project {
    /// Proyecto nuevo de la spec: una fila; cada pista con su cadena (000–007) y una frase vacía (000–007).
    pub fn new_default() -> Self {
        let tracks = default_kit().map(|(name, instrument)| Track {
            name: name.to_string(),
            instrument,
            mix: ChannelStrip::default(),
        });
        let mut project = Self {
            name: "nuevo".to_string(),
            tempo_bpm: 120.0,
            swing: 50,
            tracks,
            song: Song::default(),
            chains: vec![None; MAX_CHAINS].into_boxed_slice(),
            phrases: vec![None; MAX_PHRASES].into_boxed_slice(),
        };
        let mut row = [None; TRACKS];
        for (t, cell) in row.iter_mut().enumerate() {
            let id = t as u8;
            let mut chain = Chain::default();
            chain.entries[0] = Some(ChainEntry {
                phrase: id,
                transpose: 0,
            });
            project.chains[t] = Some(chain);
            project.phrases[t] = Some(Phrase::default());
            *cell = Some(id);
        }
        project.song.rows.push(row);
        project
    }

    pub fn chain_id_at(&self, track: usize, row: usize) -> Option<ChainId> {
        self.song.rows.get(row)?[track]
    }

    pub fn chain(&self, id: ChainId) -> Option<&Chain> {
        self.chains[usize::from(id)].as_ref()
    }

    pub fn phrase(&self, id: PhraseId) -> Option<&Phrase> {
        self.phrases[usize::from(id)].as_ref()
    }

    pub fn chain_at(&self, track: usize, row: usize) -> Option<&Chain> {
        self.chain(self.chain_id_at(track, row)?)
    }

    pub fn phrase_id_at(&self, track: usize, sec: Section) -> Option<PhraseId> {
        Some(self.chain_at(track, sec.row)?.entry(sec.entry)?.phrase)
    }

    pub fn phrase_at(&self, track: usize, sec: Section) -> Option<&Phrase> {
        self.phrase(self.phrase_id_at(track, sec)?)
    }

    pub fn step(&self, track: usize, sec: Section, step: usize) -> Option<&Step> {
        self.phrase_at(track, sec)?.steps.get(step)
    }

    pub fn step_mut(&mut self, track: usize, sec: Section, step: usize) -> Option<&mut Step> {
        let id = self.phrase_id_at(track, sec)?;
        self.phrases[usize::from(id)].as_mut()?.steps.get_mut(step)
    }

    /// Largo de la cadena más larga de la fila: cuántas secciones tiene.
    pub fn section_count(&self, row: usize) -> usize {
        (0..TRACKS)
            .filter_map(|t| self.chain_at(t, row).map(Chain::len))
            .max()
            .unwrap_or(0)
    }

    /// Última entrada navegable de la fila: las existentes más una nueva, para extender las cadenas.
    fn last_entry(&self, row: usize) -> usize {
        self.section_count(row).min(CHAIN_LEN - 1)
    }

    /// Última fila navegable: las existentes más una nueva vacía.
    fn last_row(&self) -> usize {
        self.song.rows.len().min(MAX_SONG_ROWS - 1)
    }

    /// Sección siguiente; al pasar el final de las cadenas de la fila, sigue en la fila siguiente.
    pub fn next_section(&self, sec: Section) -> Section {
        if sec.entry < self.last_entry(sec.row) {
            Section {
                row: sec.row,
                entry: sec.entry + 1,
            }
        } else if sec.row < self.last_row() {
            Section {
                row: sec.row + 1,
                entry: 0,
            }
        } else {
            sec
        }
    }

    pub fn prev_section(&self, sec: Section) -> Section {
        if sec.entry > 0 {
            Section {
                row: sec.row,
                entry: sec.entry - 1,
            }
        } else if sec.row > 0 {
            Section {
                row: sec.row - 1,
                entry: self.last_entry(sec.row - 1),
            }
        } else {
            sec
        }
    }

    pub fn free_phrase(&self) -> Option<PhraseId> {
        self.phrases
            .iter()
            .position(Option::is_none)
            .map(|i| i as PhraseId)
    }

    pub fn free_chain(&self) -> Option<ChainId> {
        self.chains
            .iter()
            .position(Option::is_none)
            .map(|i| i as ChainId)
    }

    /// Cuántas entradas de cadena apuntan a la frase.
    pub fn phrase_uses(&self, id: PhraseId) -> usize {
        self.chains
            .iter()
            .flatten()
            .flat_map(|c| c.entries.iter().flatten())
            .filter(|e| e.phrase == id)
            .count()
    }

    /// Cuántas celdas de la canción apuntan a la cadena.
    pub fn chain_uses(&self, id: ChainId) -> usize {
        self.song
            .rows
            .iter()
            .flat_map(|r| r.iter())
            .filter(|&&c| c == Some(id))
            .count()
    }

    fn ensure_phrase_exists(&mut self, id: PhraseId) {
        self.phrases[usize::from(id)].get_or_insert_with(Phrase::default);
    }

    /// Creación implícita: asegura que la pista tenga frase en la sección, creando la fila de la
    /// canción, la cadena y/o la entrada que falten. La entrada nueva usa `phrase` o una frase libre.
    /// Solo se agrega en la primera entrada libre de la cadena.
    pub fn ensure_phrase(
        &mut self,
        track: usize,
        sec: Section,
        phrase: Option<PhraseId>,
    ) -> Result<Ensured, EditError> {
        if let Some(id) = self.phrase_id_at(track, sec) {
            return Ok(Ensured {
                phrase: id,
                new_chain: None,
                added_entry: false,
            });
        }
        // Validar todo antes de modificar, para no dejar cadenas o filas a medias.
        let rows = self.song.rows.len();
        if sec.row > rows || sec.row >= MAX_SONG_ROWS {
            return Err(EditError::SongFull);
        }
        let existing_chain = self.chain_id_at(track, sec.row);
        let chain_len = existing_chain
            .and_then(|id| self.chain(id))
            .map_or(0, Chain::len);
        if sec.entry >= CHAIN_LEN {
            return Err(EditError::ChainFull);
        }
        if sec.entry > chain_len {
            return Err(EditError::Gap { chain_len });
        }
        let new_chain = match existing_chain {
            Some(_) => None,
            None => Some(self.free_chain().ok_or(EditError::NoFreeChain)?),
        };
        let phrase = match phrase {
            Some(id) => id,
            None => self.free_phrase().ok_or(EditError::NoFreePhrase)?,
        };

        if sec.row == rows {
            self.song.rows.push([None; TRACKS]);
        }
        let chain_id = match new_chain {
            Some(id) => {
                self.chains[usize::from(id)] = Some(Chain::default());
                self.song.rows[sec.row][track] = Some(id);
                id
            }
            None => existing_chain.unwrap_or_default(),
        };
        self.ensure_phrase_exists(phrase);
        let chain = self.chains[usize::from(chain_id)].get_or_insert_with(Chain::default);
        chain.entries[sec.entry] = Some(ChainEntry {
            phrase,
            transpose: 0,
        });
        Ok(Ensured {
            phrase,
            new_chain,
            added_entry: true,
        })
    }

    /// Cambia la frase de una entrada existente (la crea vacía si no existía).
    pub fn set_entry_phrase(
        &mut self,
        track: usize,
        sec: Section,
        phrase: PhraseId,
    ) -> Result<(), EditError> {
        let chain_id = self
            .chain_id_at(track, sec.row)
            .ok_or(EditError::NoPhrase)?;
        let entry = self.chains[usize::from(chain_id)]
            .as_mut()
            .and_then(|c| c.entries.get_mut(sec.entry))
            .and_then(Option::as_mut)
            .ok_or(EditError::NoPhrase)?;
        entry.phrase = phrase;
        self.ensure_phrase_exists(phrase);
        Ok(())
    }

    /// Quita la frase de la cadena (las siguientes suben). La frase sigue existiendo.
    pub fn remove_entry(&mut self, track: usize, sec: Section) -> bool {
        let Some(chain_id) = self.chain_id_at(track, sec.row) else {
            return false;
        };
        self.chains[usize::from(chain_id)]
            .as_mut()
            .is_some_and(|c| c.remove(sec.entry))
    }

    /// Pone (o vacía) la celda `(row, track)` de la canción. En la fila siguiente a la última
    /// crea la fila; si la cadena no existe, la crea vacía.
    pub fn set_song_cell(
        &mut self,
        row: usize,
        track: usize,
        chain: Option<ChainId>,
    ) -> Result<(), EditError> {
        let rows = self.song.rows.len();
        if row > rows || row >= MAX_SONG_ROWS {
            return Err(EditError::SongFull);
        }
        if row == rows {
            if chain.is_none() {
                return Ok(());
            }
            self.song.rows.push([None; TRACKS]);
        }
        if let Some(id) = chain {
            self.chains[usize::from(id)].get_or_insert_with(Chain::default);
        }
        self.song.rows[row][track] = chain;
        Ok(())
    }

    /// Copia la cadena a un número libre (las frases se comparten).
    pub fn clone_chain(&mut self, id: ChainId) -> Result<ChainId, EditError> {
        let copy = self.chain(id).cloned().ok_or(EditError::NoChain)?;
        let new_id = self.free_chain().ok_or(EditError::NoFreeChain)?;
        self.chains[usize::from(new_id)] = Some(copy);
        Ok(new_id)
    }

    /// Cambia la transposición de una entrada existente.
    pub fn set_entry_transpose(
        &mut self,
        track: usize,
        sec: Section,
        transpose: i8,
    ) -> Result<(), EditError> {
        let chain_id = self
            .chain_id_at(track, sec.row)
            .ok_or(EditError::NoPhrase)?;
        let entry = self.chains[usize::from(chain_id)]
            .as_mut()
            .and_then(|c| c.entries.get_mut(sec.entry))
            .and_then(Option::as_mut)
            .ok_or(EditError::NoPhrase)?;
        entry.transpose = transpose.clamp(-TRANSPOSE_MAX, TRANSPOSE_MAX);
        Ok(())
    }

    /// Largo (en frases) de la cadena de cada pista en la fila; `None` = celda vacía.
    pub fn chain_lengths(&self, row: usize) -> [Option<usize>; TRACKS] {
        std::array::from_fn(|t| self.chain_id_at(t, row).map(|_| self.chain_len_at(t, row)))
    }

    fn chain_len_at(&self, track: usize, row: usize) -> usize {
        self.chain_at(track, row).map_or(0, Chain::len)
    }

    /// Las cadenas de la fila tienen largos distintos: las pistas se desalinean al pasarla.
    pub fn row_misaligned(&self, row: usize) -> bool {
        let mut lens = self.chain_lengths(row).into_iter().flatten();
        let first = lens.next();
        lens.any(|l| Some(l) != first)
    }

    /// Filas del bloque de `row` en la columna de `track` (celdas no vacías contiguas).
    pub fn block_rows(&self, track: usize, row: usize) -> std::ops::Range<usize> {
        if self.chain_id_at(track, row).is_none() {
            return row..row;
        }
        let mut start = row;
        while start > 0 && self.chain_id_at(track, start - 1).is_some() {
            start -= 1;
        }
        let mut end = row + 1;
        while self.chain_id_at(track, end).is_some() {
            end += 1;
        }
        start..end
    }

    /// Frases que dura una pasada por el bloque de `row` en la columna de `track`. Una cadena
    /// sin entradas dura una frase de silencio (así lo toca el motor).
    pub fn block_phrases(&self, track: usize, row: usize) -> usize {
        self.block_rows(track, row)
            .map(|r| self.chain_len_at(track, r).max(1))
            .sum()
    }

    /// Copia la frase a un número libre.
    pub fn clone_phrase(&mut self, id: PhraseId) -> Result<PhraseId, EditError> {
        let copy = self.phrase(id).cloned().ok_or(EditError::NoPhrase)?;
        let new_id = self.free_phrase().ok_or(EditError::NoFreePhrase)?;
        self.phrases[usize::from(new_id)] = Some(copy);
        Ok(new_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S0: Section = Section { row: 0, entry: 0 };

    #[test]
    fn default_project_matches_spec() {
        let p = Project::new_default();
        assert_eq!(p.song.rows.len(), 1);
        for t in 0..TRACKS {
            assert_eq!(p.chain_id_at(t, 0), Some(t as u8));
            assert_eq!(p.phrase_id_at(t, S0), Some(t as u8));
            assert_eq!(p.phrase_at(t, S0), Some(&Phrase::default()));
        }
        assert_eq!(p.section_count(0), 1);
        assert_eq!(p.free_phrase(), Some(8));
        assert_eq!(p.free_chain(), Some(8));
        assert_eq!(p.tracks[0].name, "BOMBO");
        assert_eq!(p.tracks[7].instrument.voices, 4);
        assert_eq!(p.tracks[6].kind(), TrackKind::Synth);
    }

    #[test]
    fn note_names() {
        assert_eq!(note_name(12), "C-0");
        assert_eq!(note_name(60), "C-4");
        assert_eq!(note_name(61), "C#4");
        assert_eq!(note_name(127), "G-9");
    }

    #[test]
    fn chord_notes_and_inversions() {
        let c = |kind, inversion| Chord { kind, inversion };
        assert_eq!(c(ChordKind::None, 0).notes(60).as_slice(), &[60]);
        assert_eq!(c(ChordKind::Major, 0).notes(60).as_slice(), &[60, 64, 67]);
        assert_eq!(c(ChordKind::Major, 1).notes(60).as_slice(), &[64, 67, 72]);
        assert_eq!(c(ChordKind::Major, 2).notes(60).as_slice(), &[67, 72, 76]);
        assert_eq!(
            c(ChordKind::Min7, 3).notes(60).as_slice(),
            &[70, 72, 75, 79]
        );
        // Notas que pasan de G-9 se descartan.
        assert_eq!(c(ChordKind::Major, 0).notes(121).as_slice(), &[121, 125]);
        assert_eq!(c(ChordKind::Major, 0).max_inversion(), 2);
        assert_eq!(c(ChordKind::Major, 1).label(), "may/1");
    }

    #[test]
    fn chord_kind_cycles() {
        assert_eq!(ChordKind::None.cycle(1), ChordKind::Fifth);
        assert_eq!(ChordKind::None.cycle(-1), ChordKind::Add9);
        assert_eq!(ChordKind::Add9.cycle(1), ChordKind::None);
    }

    #[test]
    fn gates() {
        assert_eq!(gate_label(6), "1/4");
        assert_eq!(gate_label(24), "1");
        assert_eq!(gate_label(384), "16");
        assert_eq!(gate_label(GATE_INF), "INF");
        assert_eq!(gate_nudge(24, 1), 48);
        assert_eq!(gate_nudge(24, -3), 6);
        assert_eq!(gate_nudge(6, -1), 6);
        assert_eq!(gate_nudge(384, 1), GATE_INF);
        assert_eq!(gate_nudge(GATE_INF, 1), GATE_INF);
    }

    #[test]
    fn existing_phrase_is_returned() {
        let mut p = Project::new_default();
        let e = p.ensure_phrase(3, S0, None).unwrap();
        assert_eq!(e.phrase, 3);
        assert!(!e.added_entry);
        assert_eq!(e.new_chain, None);
    }

    #[test]
    fn implicit_creation_extends_chain() {
        let mut p = Project::new_default();
        let sec = Section { row: 0, entry: 1 };
        let e = p.ensure_phrase(2, sec, None).unwrap();
        assert_eq!(e.phrase, 8);
        assert!(e.added_entry);
        assert_eq!(e.new_chain, None);
        assert_eq!(p.chain_at(2, 0).unwrap().len(), 2);
        assert_eq!(p.section_count(0), 2);
        assert!(p.phrase(8).is_some());
    }

    #[test]
    fn implicit_creation_makes_chain_and_row() {
        let mut p = Project::new_default();
        let sec = Section { row: 1, entry: 0 };
        let e = p.ensure_phrase(5, sec, None).unwrap();
        assert_eq!(p.song.rows.len(), 2);
        assert_eq!(e.new_chain, Some(8));
        assert_eq!(p.chain_id_at(5, 1), Some(8));
        assert_eq!(p.phrase_id_at(5, sec), Some(e.phrase));
        // Las demás pistas de la fila nueva siguen vacías.
        assert_eq!(p.chain_id_at(0, 1), None);
    }

    #[test]
    fn implicit_creation_refuses_gaps() {
        let mut p = Project::new_default();
        let before = p.clone();
        let err = p.ensure_phrase(0, Section { row: 0, entry: 2 }, None);
        assert_eq!(err, Err(EditError::Gap { chain_len: 1 }));
        assert_eq!(p, before, "un error no deja nada a medias");
        let err = p.ensure_phrase(0, Section { row: 3, entry: 0 }, None);
        assert_eq!(err, Err(EditError::SongFull));
    }

    #[test]
    fn implicit_creation_with_preferred_phrase_shares_it() {
        let mut p = Project::new_default();
        let sec = Section { row: 0, entry: 1 };
        p.ensure_phrase(0, sec, Some(4)).unwrap();
        assert_eq!(p.phrase_id_at(0, sec), Some(4));
        assert_eq!(p.phrase_uses(4), 2);
    }

    #[test]
    fn out_of_chains_is_an_error() {
        let mut p = Project::new_default();
        for c in p.chains.iter_mut() {
            c.get_or_insert_with(Chain::default);
        }
        let err = p.ensure_phrase(0, Section { row: 1, entry: 0 }, None);
        assert_eq!(err, Err(EditError::NoFreeChain));
        assert_eq!(p.song.rows.len(), 1);
    }

    #[test]
    fn remove_entry_shifts_up() {
        let mut p = Project::new_default();
        p.ensure_phrase(1, Section { row: 0, entry: 1 }, Some(9))
            .unwrap();
        p.ensure_phrase(1, Section { row: 0, entry: 2 }, Some(10))
            .unwrap();
        assert!(p.remove_entry(1, Section { row: 0, entry: 1 }));
        let chain = p.chain_at(1, 0).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.entry(1).unwrap().phrase, 10);
        assert!(!p.remove_entry(1, Section { row: 0, entry: 5 }));
    }

    #[test]
    fn set_entry_and_clone() {
        let mut p = Project::new_default();
        p.set_entry_phrase(0, S0, 42).unwrap();
        assert!(p.phrase(42).is_some());
        assert_eq!(p.phrase_uses(0), 0);
        p.step_mut(0, S0, 0).unwrap().note = NoteCell::On(60);
        let copy = p.clone_phrase(42).unwrap();
        assert_eq!(copy, 8);
        assert_eq!(p.phrase(copy).unwrap().steps[0].note, NoteCell::On(60));
    }

    #[test]
    fn section_navigation() {
        let mut p = Project::new_default();
        // Fila 0 con una sección: navega a la entrada 1 (nueva) y luego a la fila 1 (nueva).
        let s1 = p.next_section(S0);
        assert_eq!(s1, Section { row: 0, entry: 1 });
        let s2 = p.next_section(s1);
        assert_eq!(s2, Section { row: 1, entry: 0 });
        assert_eq!(p.next_section(s2), s2, "no pasa de la fila nueva");
        assert_eq!(p.prev_section(s2), s1);
        assert_eq!(p.prev_section(S0), S0);

        p.ensure_phrase(0, s1, None).unwrap();
        assert_eq!(p.section_count(0), 2);
        assert_eq!(
            p.next_section(s1),
            Section { row: 0, entry: 2 },
            "la nueva sección pasa a ser la 2"
        );
    }

    #[test]
    fn song_cells_rows_and_clones() {
        let mut p = Project::new_default();
        assert_eq!(p.set_song_cell(2, 0, Some(3)), Err(EditError::SongFull));
        p.set_song_cell(1, 0, None).unwrap();
        assert_eq!(p.song.rows.len(), 1, "vaciar la fila nueva no la crea");
        p.set_song_cell(1, 2, Some(40)).unwrap();
        assert_eq!(p.song.rows.len(), 2);
        assert_eq!(p.chain(40).map(Chain::len), Some(0), "cadena nueva vacía");
        let copy = p.clone_chain(0).unwrap();
        assert_eq!(copy, 8);
        assert_eq!(p.chain(copy), p.chain(0));
        assert_eq!(p.clone_chain(200), Err(EditError::NoChain));

        p.set_entry_transpose(0, S0, 60).unwrap();
        assert_eq!(p.chain_at(0, 0).unwrap().entry(0).unwrap().transpose, 48);
        assert_eq!(
            p.set_entry_transpose(0, Section { row: 0, entry: 3 }, 1),
            Err(EditError::NoPhrase)
        );
    }

    #[test]
    fn lengths_alignment_and_blocks() {
        let mut p = Project::new_default();
        assert!(!p.row_misaligned(0));
        p.ensure_phrase(7, Section { row: 0, entry: 1 }, None)
            .unwrap();
        let lens = p.chain_lengths(0);
        assert_eq!(lens[0], Some(1));
        assert_eq!(lens[7], Some(2));
        assert!(p.row_misaligned(0));
        // Fila 1 con solo la pista 0; fila 2 vacía; fila 3 con la pista 0 (otro bloque).
        p.set_song_cell(1, 0, Some(0)).unwrap();
        p.set_song_cell(2, 1, Some(1)).unwrap();
        p.set_song_cell(3, 0, Some(9)).unwrap();
        assert!(!p.row_misaligned(1), "una sola cadena: alineada");
        assert_eq!(p.chain_lengths(1)[1], None);
        assert_eq!(p.block_rows(0, 1), 0..2);
        assert_eq!(p.block_rows(0, 3), 3..4);
        assert_eq!(p.block_rows(0, 2), 2..2);
        assert_eq!(p.block_phrases(0, 0), 2);
        assert_eq!(
            p.block_phrases(0, 3),
            1,
            "cadena vacía: una frase de silencio"
        );
        assert_eq!(p.block_phrases(7, 0), 2);
    }

    #[test]
    fn uses_are_counted() {
        let mut p = Project::new_default();
        p.song.rows.push([Some(0); TRACKS]);
        assert_eq!(p.chain_uses(0), 9);
        assert_eq!(p.chain_uses(1), 1);
        assert_eq!(p.phrase_uses(0), 1);
    }
}
