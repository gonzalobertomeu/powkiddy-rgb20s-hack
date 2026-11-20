//! Estado de la interfaz y edición del proyecto a partir de gestos. Sin SDL ni audio:
//! `handle` devuelve, como mucho, una audición para mandarle al motor.

use crate::engine::{HeadPos, LoadReport, PlayMode, Playhead};
use crate::gestures::{Dir, Gesture};
use crate::model::{
    CHAIN_LEN, ChainId, Chord, DEFAULT_GATE, DEFAULT_VELOCITY, EditError, MAX_SONG_ROWS, NOTE_MAX,
    NOTE_MIN, NoteCell, NoteSet, PHRASE_STEPS, PhraseId, Project, Section, Step, TRACKS,
    TRANSPOSE_MAX, TrackKind, gate_nudge,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Song,
    Grid,
    Chain,
    Phrase,
    Instrument,
    Project,
    Mix,
}

impl Screen {
    /// Fila del medio del mapa de pantallas, de lo general a lo particular.
    pub const MIDDLE: [Screen; 5] = [
        Screen::Song,
        Screen::Grid,
        Screen::Chain,
        Screen::Phrase,
        Screen::Instrument,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Screen::Song => "CANCIÓN",
            Screen::Grid => "GRILLA",
            Screen::Chain => "CADENA",
            Screen::Phrase => "FRASE",
            Screen::Instrument => "INSTRUMENTO",
            Screen::Project => "PROYECTO",
            Screen::Mix => "MEZCLA",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Screen::Song => "CAN",
            Screen::Grid => "GRI",
            Screen::Chain => "CAD",
            Screen::Phrase => "FRA",
            Screen::Instrument => "INS",
            Screen::Project => "PRY",
            Screen::Mix => "MEZ",
        }
    }
}

/// Columna del cursor en la Grilla: FRA o un paso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridCol {
    Phrase,
    Step(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhraseCol {
    Note,
    Chord,
    Velocity,
    Gate,
}

impl PhraseCol {
    pub const ALL: [PhraseCol; 4] = [
        PhraseCol::Note,
        PhraseCol::Chord,
        PhraseCol::Velocity,
        PhraseCol::Gate,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

/// Columna del cursor en la Cadena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainCol {
    Phrase,
    Transpose,
}

/// Filas que salta B + ↑/↓ y L2/R2 en la Canción.
pub const SONG_PAGE: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontSize {
    /// 8×16
    Small,
    /// 12×24
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectItem {
    Font,
    AuditionOnEdit,
    Follow,
    Quit,
}

impl ProjectItem {
    pub const ALL: [ProjectItem; 4] = [
        ProjectItem::Font,
        ProjectItem::AuditionOnEdit,
        ProjectItem::Follow,
        ProjectItem::Quit,
    ];
}

/// Últimos valores usados en una pista: lo que pone A en un paso vacío.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackMemory {
    pub note: u8,
    pub chord: Chord,
    pub velocity: u8,
    pub gate: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Audition {
    pub track: usize,
    pub notes: NoteSet,
    pub velocity: u8,
    /// Largo en ticks (`GATE_INF` = hasta la próxima nota); la percusión lo ignora.
    pub gate: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub text: String,
    pub at_ms: u64,
    pub warn: bool,
}

pub const MESSAGE_MS: u64 = 3000;
const DEFAULT_OCTAVE: u8 = 4;
const TEMPO_RANGE: (f32, f32) = (40.0, 300.0);

pub struct App {
    pub project: Project,
    pub screen: Screen,
    /// Pantalla a la que vuelven Proyecto y Mezcla.
    pub origin: Screen,
    pub track: usize,
    pub section: Section,
    pub grid_col: GridCol,
    pub phrase_row: usize,
    pub phrase_col: PhraseCol,
    pub chain_col: ChainCol,
    pub project_item: usize,
    pub memory: [TrackMemory; TRACKS],
    pub octave: u8,
    pub last_phrase: Option<PhraseId>,
    pub last_chain: Option<ChainId>,
    pub font: FontSize,
    pub audition_on_edit: bool,
    /// "Seguir reproducción": la Grilla y la Frase muestran lo que suena.
    pub follow: bool,
    /// Último reporte de carga del motor (None sin audio).
    pub audio_load: Option<LoadReport>,
    /// Lo que se le pidió al motor que reproduzca (None = parado).
    pub transport: Option<PlayMode>,
    /// Última posición de los cabezales que publicó el motor (se actualiza con `set_playhead`).
    pub playhead: Playhead,
    pub modified: bool,
    pub message: Option<Message>,
    pub quit: bool,
    now: u64,
    /// El último gesto fue un A que puso un número de frase o de cadena (para el doble toque).
    just_inserted: bool,
}

fn dir_delta(dir: Dir, fine: i32, coarse: i32, fast: bool) -> i32 {
    let d = match dir {
        Dir::Right => fine,
        Dir::Left => -fine,
        Dir::Up => coarse,
        Dir::Down => -coarse,
    };
    if fast { d * 4 } else { d }
}

fn step_by(value: usize, delta: i32, max: usize) -> usize {
    (value as i32 + delta).clamp(0, max as i32) as usize
}

/// El paso con la transposición de su entrada de cadena (como lo toca el motor).
fn transposed(step: &Step, transpose: i32) -> Step {
    let mut s = *step;
    if let NoteCell::On(n) = s.note {
        s.note =
            NoteCell::On((i32::from(n) + transpose).clamp(NOTE_MIN.into(), NOTE_MAX.into()) as u8);
    }
    s
}

impl App {
    pub fn new(project: Project) -> Self {
        let memory = TrackMemory {
            note: (DEFAULT_OCTAVE + 1) * 12,
            chord: Chord::default(),
            velocity: DEFAULT_VELOCITY,
            gate: DEFAULT_GATE,
        };
        Self {
            project,
            screen: Screen::Grid,
            origin: Screen::Grid,
            track: 0,
            section: Section::default(),
            grid_col: GridCol::Step(0),
            phrase_row: 0,
            phrase_col: PhraseCol::Note,
            chain_col: ChainCol::Phrase,
            project_item: 0,
            memory: [memory; TRACKS],
            octave: DEFAULT_OCTAVE,
            last_phrase: None,
            last_chain: None,
            font: FontSize::Small,
            audition_on_edit: true,
            follow: true,
            audio_load: None,
            transport: None,
            playhead: Playhead::default(),
            modified: false,
            message: None,
            quit: false,
            now: 0,
            just_inserted: false,
        }
    }

    /// Mensaje vigente (dura `MESSAGE_MS`).
    pub fn message_at(&self, now: u64) -> Option<&Message> {
        self.message
            .as_ref()
            .filter(|m| now.saturating_sub(m.at_ms) < MESSAGE_MS)
    }

    fn info(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            at_ms: self.now,
            warn: false,
        });
    }

    fn warn(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            at_ms: self.now,
            warn: true,
        });
    }

    pub fn track_kind(&self) -> TrackKind {
        self.project.tracks[self.track].kind()
    }

    /// Cabezal de `track` si está sonando.
    pub fn head(&self, track: usize) -> Option<HeadPos> {
        let h = *self.playhead.heads.get(track)?;
        (self.playhead.playing && h.active).then_some(h)
    }

    /// La Grilla y la Frase siguen a los cabezales.
    fn following(&self) -> bool {
        self.follow && self.playhead.playing && matches!(self.screen, Screen::Grid | Screen::Phrase)
    }

    /// Sección que se muestra para `track`. Siguiendo la reproducción, cada pista muestra la
    /// sección donde está su cabezal (spec-ui, Grilla); la del cursor ya la sigue `section`.
    pub fn shown_section(&self, track: usize) -> Section {
        match self.head(track) {
            Some(h) if self.following() && track != self.track => Section {
                row: usize::from(h.row),
                entry: usize::from(h.entry),
            },
            _ => self.section,
        }
    }

    /// Paso que está sonando en `track` si su cabezal está en la sección que se ve.
    pub fn playing_step(&self, track: usize) -> Option<usize> {
        let h = self.head(track)?;
        let sec = self.shown_section(track);
        (usize::from(h.row) == sec.row && usize::from(h.entry) == sec.entry)
            .then_some(usize::from(h.step))
    }

    /// Nueva posición de los cabezales. Siguiendo la reproducción, la sección salta a la del
    /// cabezal de la pista actual solo cuando ese cabezal cambia de sección: entre medio, L2/R2
    /// pueden mirar otra sección hasta el próximo cambio.
    pub fn set_playhead(&mut self, p: Playhead) {
        let old = self.head(self.track);
        self.playhead = p;
        let moved = match (old, self.head(self.track)) {
            (Some(o), Some(h)) => (o.row, o.entry) != (h.row, h.entry),
            (None, Some(_)) => true,
            _ => false,
        };
        if moved {
            self.snap_to_head();
        }
    }

    fn snap_to_head(&mut self) {
        if self.following()
            && let Some(h) = self.head(self.track)
        {
            self.section = Section {
                row: usize::from(h.row),
                entry: usize::from(h.entry),
            };
        }
    }

    /// Lo que reproduce Start en la pantalla actual (spec-ui, "Qué reproduce Start").
    pub fn start_mode(&self) -> PlayMode {
        let row = self.section.row.min(MAX_SONG_ROWS - 1) as u8;
        let track = self.track as u8;
        match self.screen {
            Screen::Song | Screen::Grid | Screen::Mix | Screen::Project => {
                PlayMode::Song { from_row: row }
            }
            Screen::Chain => PlayMode::Chain { track, row },
            Screen::Phrase | Screen::Instrument => PlayMode::Phrase {
                track,
                row,
                entry: self.section.entry as u8,
            },
        }
    }

    fn section_mode(&self) -> PlayMode {
        PlayMode::Section {
            row: self.section.row.min(MAX_SONG_ROWS - 1) as u8,
            entry: self.section.entry as u8,
        }
    }

    /// Paso bajo el cursor en la Grilla o la Frase.
    pub fn cursor_step(&self) -> Option<usize> {
        match (self.screen, self.grid_col) {
            (Screen::Grid, GridCol::Step(i)) => Some(i),
            (Screen::Phrase, _) => Some(self.phrase_row),
            _ => None,
        }
    }

    pub fn handle(&mut self, g: Gesture, now: u64) -> Option<Audition> {
        self.now = now;
        let just_inserted = std::mem::take(&mut self.just_inserted);
        let track = self.track;
        let screen = self.screen;
        let audition = self.dispatch(g, just_inserted);
        if self.track != track || self.screen != screen {
            self.snap_to_head();
        }
        audition
    }

    fn dispatch(&mut self, g: Gesture, just_inserted: bool) -> Option<Audition> {
        match g {
            Gesture::Screen(dir) => {
                self.navigate(dir);
                None
            }
            Gesture::Quit => {
                self.quit = true;
                None
            }
            Gesture::Mute | Gesture::Solo => {
                let track = &mut self.project.tracks[self.track];
                let (label, on) = if g == Gesture::Mute {
                    track.mix.mute = !track.mix.mute;
                    ("mute", track.mix.mute)
                } else {
                    track.mix.solo = !track.mix.solo;
                    ("solo", track.mix.solo)
                };
                let text = format!(
                    "pista {} {}: {label} {}",
                    self.track + 1,
                    track.name,
                    if on { "sí" } else { "no" }
                );
                self.modified = true;
                self.info(text);
                None
            }
            Gesture::Octave(d) => {
                self.set_octave(self.octave as i32 + d);
                None
            }
            Gesture::Tempo(d) => {
                let (lo, hi) = TEMPO_RANGE;
                self.project.tempo_bpm = (self.project.tempo_bpm + d as f32).clamp(lo, hi);
                self.modified = true;
                None
            }
            // Start reproduce según la pantalla; Select + Start, el loop de la sección. Con algo
            // sonando, los dos paran.
            Gesture::Play | Gesture::LoopSection => {
                self.transport = match self.transport {
                    Some(_) => None,
                    None if g == Gesture::Play => Some(self.start_mode()),
                    None => Some(self.section_mode()),
                };
                None
            }
            Gesture::Paste | Gesture::SelectMode => {
                self.info("selección, copiar y pegar: llegan en H10");
                None
            }
            Gesture::Track(d) if self.screen != Screen::Project => {
                self.track = step_by(self.track, d, TRACKS - 1);
                None
            }
            _ => match self.screen {
                Screen::Song => self.handle_song(g, just_inserted),
                Screen::Grid => self.handle_grid(g, just_inserted),
                Screen::Chain => self.handle_chain(g, just_inserted),
                Screen::Phrase => self.handle_phrase(g),
                Screen::Project => self.handle_project(g),
                _ => self.handle_other(g),
            },
        }
    }

    fn navigate(&mut self, dir: Dir) {
        match self.screen {
            Screen::Project if dir == Dir::Down => self.go(self.origin),
            Screen::Mix if dir == Dir::Up => self.go(self.origin),
            Screen::Project | Screen::Mix => {}
            current => {
                let i = Screen::MIDDLE
                    .iter()
                    .position(|&s| s == current)
                    .unwrap_or(1);
                match dir {
                    Dir::Left if i > 0 => self.go(Screen::MIDDLE[i - 1]),
                    Dir::Right if i + 1 < Screen::MIDDLE.len() => self.go(Screen::MIDDLE[i + 1]),
                    Dir::Up => {
                        self.origin = current;
                        self.go(Screen::Project);
                    }
                    Dir::Down => {
                        self.origin = current;
                        self.go(Screen::Mix);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Cambia de pantalla con el contexto: la Grilla y la Frase comparten el paso del cursor y,
    /// desde la Canción, la Grilla abre la sección `(R, 0)` (spec-ui, transiciones).
    fn go(&mut self, to: Screen) {
        if self.screen == Screen::Song && to == Screen::Grid {
            self.section.entry = 0;
        }
        match self.screen {
            Screen::Grid => {
                if let GridCol::Step(i) = self.grid_col {
                    self.phrase_row = i;
                }
            }
            Screen::Phrase => {
                if let GridCol::Step(_) = self.grid_col {
                    self.grid_col = GridCol::Step(self.phrase_row);
                }
            }
            _ => {}
        }
        self.screen = to;
    }

    fn set_octave(&mut self, octave: i32) {
        self.octave = octave.clamp(0, 9) as u8;
        for m in &mut self.memory {
            let note = (i32::from(self.octave) + 1) * 12 + i32::from(m.note % 12);
            m.note = note.clamp(NOTE_MIN.into(), NOTE_MAX.into()) as u8;
        }
        self.info(format!("octava base {} para notas nuevas", self.octave));
    }

    fn notes_of(&self, track: usize, step: &Step) -> Option<NoteSet> {
        let root = step.root()?;
        Some(if self.project.tracks[track].kind() == TrackKind::Drum {
            NoteSet::single(root)
        } else {
            step.chord.notes(root)
        })
    }

    /// X: el paso bajo el cursor, o el instrumento con la última nota si está vacío.
    fn listen(&self, step: Option<usize>) -> Option<Audition> {
        self.listen_at(self.section, step)
    }

    /// X en la Canción y la Cadena: la primera nota de la frase de la sección (o el instrumento).
    fn listen_phrase(&self, sec: Section) -> Option<Audition> {
        let first = self
            .project
            .phrase_at(self.track, sec)
            .and_then(|p| p.steps.iter().position(|s| s.root().is_some()));
        self.listen_at(sec, first)
    }

    fn listen_at(&self, sec: Section, step: Option<usize>) -> Option<Audition> {
        let transpose = self
            .project
            .chain_at(self.track, sec.row)
            .and_then(|c| c.entry(sec.entry))
            .map_or(0, |e| i32::from(e.transpose));
        let s = step
            .and_then(|i| self.project.step(self.track, sec, i))
            .map(|s| transposed(s, transpose));
        if let Some(s) = s
            && let Some(notes) = self.notes_of(self.track, &s)
        {
            return Some(Audition {
                track: self.track,
                notes,
                velocity: s.velocity,
                gate: s.gate_ticks,
            });
        }
        let m = self.memory[self.track];
        let notes = if self.track_kind() == TrackKind::Drum {
            NoteSet::single(m.note)
        } else {
            m.chord.notes(m.note)
        };
        Some(Audition {
            track: self.track,
            notes,
            velocity: m.velocity,
            gate: m.gate,
        })
    }

    fn edit_audition(&self, step: usize) -> Option<Audition> {
        if self.audition_on_edit {
            self.listen(Some(step))
        } else {
            None
        }
    }

    fn remember(&mut self, step: Step) {
        let m = &mut self.memory[self.track];
        if let NoteCell::On(n) = step.note {
            m.note = n;
        }
        if self.project.tracks[self.track].kind() != TrackKind::Drum {
            m.chord = step.chord;
        }
        m.velocity = step.velocity;
        m.gate = step.gate_ticks;
    }

    /// Creación implícita (spec-ui, Grilla): crea lo que falte para tener frase en la sección.
    fn ensure_phrase(&mut self, phrase: Option<PhraseId>) -> Result<PhraseId, EditError> {
        let e = self
            .project
            .ensure_phrase(self.track, self.section, phrase)?;
        if let Some(chain) = e.new_chain {
            self.last_chain = Some(chain);
            self.info(format!("cadena {chain:03} y frase {:03} creadas", e.phrase));
        } else if e.added_entry {
            self.info(format!("frase {:03} agregada a la cadena", e.phrase));
        }
        if e.added_entry {
            self.modified = true;
            self.last_phrase = Some(e.phrase);
        }
        Ok(e.phrase)
    }

    /// A sobre un paso: si está vacío pone la última nota, acorde, velocidad y largo de la pista.
    fn insert_step(&mut self, i: usize) -> Option<Audition> {
        if let Err(e) = self.ensure_phrase(None) {
            self.warn(e.to_string());
            return None;
        }
        let m = self.memory[self.track];
        let drum = self.track_kind() == TrackKind::Drum;
        let step = self.project.step_mut(self.track, self.section, i)?;
        if step.is_empty() {
            *step = Step {
                note: NoteCell::On(m.note),
                chord: if drum { Chord::default() } else { m.chord },
                velocity: m.velocity,
                gate_ticks: m.gate,
            };
            self.modified = true;
        }
        self.edit_audition(i)
    }

    fn nudge_note(&mut self, i: usize, dir: Dir, fast: bool) -> Option<Audition> {
        let delta = dir_delta(dir, 1, 12, fast);
        let step = self.project.step_mut(self.track, self.section, i)?;
        let NoteCell::On(n) = step.note else {
            return None;
        };
        let n = (i32::from(n) + delta).clamp(NOTE_MIN.into(), NOTE_MAX.into()) as u8;
        step.note = NoteCell::On(n);
        let copy = *step;
        self.modified = true;
        self.remember(copy);
        self.edit_audition(i)
    }

    fn delete_step(&mut self, i: usize) {
        if let Some(step) = self.project.step_mut(self.track, self.section, i)
            && !step.is_empty()
        {
            *step = Step::default();
            self.modified = true;
        }
    }

    fn chord_step(&mut self, i: usize, delta: i32) -> Option<Audition> {
        if self.track_kind() == TrackKind::Drum {
            self.warn("los acordes se ignoran en las pistas de percusión");
            return None;
        }
        let step = self.project.step_mut(self.track, self.section, i)?;
        step.root()?;
        step.chord.kind = step.chord.kind.cycle(delta);
        step.chord.inversion = step.chord.inversion.min(step.chord.max_inversion());
        let copy = *step;
        self.modified = true;
        self.remember(copy);
        self.edit_audition(i)
    }

    fn clone_current_phrase(&mut self) {
        let Some(id) = self.project.phrase_id_at(self.track, self.section) else {
            self.warn(EditError::NoPhrase.to_string());
            return;
        };
        match self.project.clone_phrase(id) {
            Ok(new) => {
                // La entrada existe: el id salió de ella.
                let _ = self.project.set_entry_phrase(self.track, self.section, new);
                self.last_phrase = Some(new);
                self.modified = true;
                self.info(format!("frase {id:03} clonada en {new:03}"));
            }
            Err(e) => self.warn(e.to_string()),
        }
    }

    fn handle_grid(&mut self, g: Gesture, just_inserted: bool) -> Option<Audition> {
        let col = self.grid_col;
        match (g, col) {
            (Gesture::Move(dir), _) => {
                match dir {
                    Dir::Up => self.track = self.track.saturating_sub(1),
                    Dir::Down => self.track = (self.track + 1).min(TRACKS - 1),
                    Dir::Left => {
                        self.grid_col = match col {
                            GridCol::Step(0) | GridCol::Phrase => GridCol::Phrase,
                            GridCol::Step(i) => GridCol::Step(i - 1),
                        }
                    }
                    Dir::Right => {
                        self.grid_col = match col {
                            GridCol::Phrase => GridCol::Step(0),
                            GridCol::Step(i) => GridCol::Step((i + 1).min(PHRASE_STEPS - 1)),
                        }
                    }
                }
                None
            }
            (Gesture::Jump(dir), _) => {
                match dir {
                    Dir::Up => self.track = 0,
                    Dir::Down => self.track = TRACKS - 1,
                    Dir::Left => self.grid_col = GridCol::Step(0),
                    Dir::Right => self.grid_col = GridCol::Step(PHRASE_STEPS - 1),
                }
                None
            }
            (Gesture::Section(d), _) => {
                self.section = if d > 0 {
                    self.project.next_section(self.section)
                } else {
                    self.project.prev_section(self.section)
                };
                None
            }
            (Gesture::Listen, _) => self.listen(self.cursor_step()),

            (Gesture::Insert | Gesture::InsertNew, GridCol::Step(i)) => self.insert_step(i),
            (Gesture::Edit { dir, fast }, GridCol::Step(i)) => self.nudge_note(i, dir, fast),
            (Gesture::Delete, GridCol::Step(i)) => {
                self.delete_step(i);
                None
            }
            (Gesture::Chord(d), GridCol::Step(i)) => self.chord_step(i, d),

            (Gesture::Insert | Gesture::InsertNew | Gesture::Edit { .. }, GridCol::Phrase)
            | (Gesture::Delete | Gesture::Clone, GridCol::Phrase) => {
                self.fra_gesture(g, just_inserted);
                None
            }
            _ => None,
        }
    }

    /// Gestos sobre el número de frase de la sección (FRA en la Grilla, FRASE en la Cadena).
    fn fra_gesture(&mut self, g: Gesture, just_inserted: bool) {
        let current = self.project.phrase_id_at(self.track, self.section);
        match g {
            Gesture::Insert => match current {
                None => match self.ensure_phrase(self.last_phrase) {
                    Ok(_) => self.just_inserted = true,
                    Err(e) => self.warn(e.to_string()),
                },
                Some(id) => self.last_phrase = Some(id),
            },
            Gesture::InsertNew => {
                // Si el primer toque ya creó una frase nueva (no había última), se queda esa.
                let fresh = current.is_some_and(|id| {
                    self.project.phrase_uses(id) == 1
                        && self
                            .project
                            .phrase(id)
                            .is_some_and(|p| p.steps.iter().all(Step::is_empty))
                });
                let result = match (current, just_inserted, self.project.free_phrase()) {
                    (Some(_), false, _) => return,
                    (Some(id), true, _) if fresh => Ok(id),
                    (_, _, None) => Err(EditError::NoFreePhrase),
                    (Some(_), true, Some(new)) => self
                        .project
                        .set_entry_phrase(self.track, self.section, new)
                        .map(|()| new),
                    (None, _, Some(new)) => self.ensure_phrase(Some(new)),
                };
                match result {
                    Ok(new) => {
                        self.last_phrase = Some(new);
                        self.modified = true;
                        self.info(format!("frase nueva {new:03}"));
                    }
                    Err(e) => self.warn(e.to_string()),
                }
            }
            Gesture::Edit { dir, fast } => {
                let Some(id) = current else { return };
                let id = (i32::from(id) + dir_delta(dir, 1, 10, fast)).clamp(0, 255) as PhraseId;
                if self
                    .project
                    .set_entry_phrase(self.track, self.section, id)
                    .is_ok()
                {
                    self.last_phrase = Some(id);
                    self.modified = true;
                }
            }
            Gesture::Delete => {
                if self.project.remove_entry(self.track, self.section) {
                    self.modified = true;
                    self.info("frase quitada de la cadena");
                }
            }
            Gesture::Clone => self.clone_current_phrase(),
            _ => {}
        }
    }

    /// Fila de la canción bajo el cursor (en la Canción y la Cadena); la sección vuelve a la
    /// primera entrada si la nueva fila no la tiene.
    fn set_row(&mut self, row: usize) {
        let row = row.min(self.last_row());
        if row != self.section.row {
            self.section.row = row;
            let len = self
                .project
                .chain_at(self.track, row)
                .map_or(0, |c| c.len());
            if self.screen == Screen::Song || self.section.entry > len {
                self.section.entry = 0;
            }
        }
    }

    /// Última fila navegable: las existentes más una nueva.
    fn last_row(&self) -> usize {
        self.project.song.rows.len().min(MAX_SONG_ROWS - 1)
    }

    fn set_cell(&mut self, chain: Option<ChainId>) -> Result<(), EditError> {
        self.project
            .set_song_cell(self.section.row, self.track, chain)?;
        if chain.is_some() {
            self.last_chain = chain;
        }
        self.modified = true;
        Ok(())
    }

    fn handle_song(&mut self, g: Gesture, just_inserted: bool) -> Option<Audition> {
        let row = self.section.row;
        let current = self.project.chain_id_at(self.track, row);
        let result = match g {
            Gesture::Move(dir) => {
                match dir {
                    Dir::Up => self.set_row(row.saturating_sub(1)),
                    Dir::Down => self.set_row(row + 1),
                    Dir::Left => self.track = self.track.saturating_sub(1),
                    Dir::Right => self.track = (self.track + 1).min(TRACKS - 1),
                }
                Ok(())
            }
            Gesture::Jump(dir) => {
                match dir {
                    Dir::Up => self.set_row(row.saturating_sub(SONG_PAGE)),
                    Dir::Down => self.set_row(row + SONG_PAGE),
                    Dir::Left => self.track = 0,
                    Dir::Right => self.track = TRACKS - 1,
                }
                Ok(())
            }
            Gesture::Section(d) => {
                self.set_row(step_by(row, d * SONG_PAGE as i32, MAX_SONG_ROWS - 1));
                Ok(())
            }
            Gesture::Listen => {
                return self.listen_phrase(Section { row, entry: 0 });
            }
            Gesture::Insert => match current {
                Some(id) => {
                    self.last_chain = Some(id);
                    Ok(())
                }
                None => {
                    let id = match self.last_chain {
                        Some(id) => Some(id),
                        None => self.project.free_chain(),
                    };
                    match id {
                        Some(id) => self.set_cell(Some(id)).map(|()| {
                            self.just_inserted = true;
                            self.info(format!("cadena {id:03}"));
                        }),
                        None => Err(EditError::NoFreeChain),
                    }
                }
            },
            Gesture::InsertNew => {
                let fresh = current.is_some_and(|id| {
                    self.project.chain_uses(id) == 1
                        && self.project.chain(id).is_some_and(|c| c.len() == 0)
                });
                match (current, just_inserted, self.project.free_chain()) {
                    (Some(_), false, _) => Ok(()),
                    (Some(id), true, _) if fresh => {
                        self.info(format!("cadena nueva {id:03}"));
                        Ok(())
                    }
                    (_, _, None) => Err(EditError::NoFreeChain),
                    (_, _, Some(new)) => self.set_cell(Some(new)).map(|()| {
                        self.info(format!("cadena nueva {new:03}"));
                    }),
                }
            }
            Gesture::Edit { dir, fast } => match current {
                Some(id) => {
                    let id = (i32::from(id) + dir_delta(dir, 1, 10, fast)).clamp(0, 255) as ChainId;
                    self.set_cell(Some(id))
                }
                None => Ok(()),
            },
            Gesture::Delete => match current {
                Some(_) => self.set_cell(None),
                None => Ok(()),
            },
            Gesture::Clone => match current {
                Some(id) => self.project.clone_chain(id).and_then(|new| {
                    self.set_cell(Some(new))?;
                    self.info(format!("cadena {id:03} clonada en {new:03}"));
                    Ok(())
                }),
                None => Err(EditError::NoChain),
            },
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.warn(e.to_string());
        }
        None
    }

    fn handle_chain(&mut self, g: Gesture, just_inserted: bool) -> Option<Audition> {
        let entry = self.section.entry;
        match (g, self.chain_col) {
            (Gesture::Move(dir), _) => match dir {
                Dir::Up => self.section.entry = entry.saturating_sub(1),
                Dir::Down => self.section.entry = (entry + 1).min(CHAIN_LEN - 1),
                Dir::Left => self.chain_col = ChainCol::Phrase,
                Dir::Right => self.chain_col = ChainCol::Transpose,
            },
            (Gesture::Jump(dir), _) => match dir {
                Dir::Up => self.section.entry = 0,
                Dir::Down => self.section.entry = CHAIN_LEN - 1,
                Dir::Left => self.chain_col = ChainCol::Phrase,
                Dir::Right => self.chain_col = ChainCol::Transpose,
            },
            (Gesture::Section(d), _) => {
                self.set_row(step_by(self.section.row, d, MAX_SONG_ROWS - 1));
            }
            (Gesture::Listen, _) => return self.listen_phrase(self.section),
            (Gesture::Edit { dir, fast }, ChainCol::Transpose) => {
                let e = self
                    .project
                    .chain_at(self.track, self.section.row)
                    .and_then(|c| c.entry(entry))?;
                let max = i32::from(TRANSPOSE_MAX);
                let t = (i32::from(e.transpose) + dir_delta(dir, 1, 12, fast)).clamp(-max, max);
                if self
                    .project
                    .set_entry_transpose(self.track, self.section, t as i8)
                    .is_ok()
                {
                    self.modified = true;
                }
            }
            (Gesture::Delete, ChainCol::Transpose) => {
                if self
                    .project
                    .set_entry_transpose(self.track, self.section, 0)
                    .is_ok()
                {
                    self.modified = true;
                }
            }
            (Gesture::Insert | Gesture::InsertNew, ChainCol::Transpose) => {}
            (
                Gesture::Insert
                | Gesture::InsertNew
                | Gesture::Edit { .. }
                | Gesture::Delete
                | Gesture::Clone,
                _,
            ) => self.fra_gesture(g, just_inserted),
            _ => {}
        }
        None
    }

    fn handle_phrase(&mut self, g: Gesture) -> Option<Audition> {
        let row = self.phrase_row;
        let last_col = PhraseCol::ALL.len() - 1;
        match g {
            Gesture::Move(dir) => {
                match dir {
                    Dir::Up => self.phrase_row = row.saturating_sub(1),
                    Dir::Down => self.phrase_row = (row + 1).min(PHRASE_STEPS - 1),
                    Dir::Left | Dir::Right => {
                        let d = if dir == Dir::Left { -1 } else { 1 };
                        self.phrase_col =
                            PhraseCol::ALL[step_by(self.phrase_col.index(), d, last_col)];
                    }
                }
                None
            }
            Gesture::Jump(dir) => {
                match dir {
                    Dir::Up => self.phrase_row = 0,
                    Dir::Down => self.phrase_row = PHRASE_STEPS - 1,
                    Dir::Left => self.phrase_col = PhraseCol::ALL[0],
                    Dir::Right => self.phrase_col = PhraseCol::ALL[last_col],
                }
                None
            }
            Gesture::Section(d) => {
                let len = self
                    .project
                    .chain_at(self.track, self.section.row)
                    .map_or(0, |c| c.len());
                let max = len.min(crate::model::CHAIN_LEN - 1).max(self.section.entry);
                self.section.entry = step_by(self.section.entry, d, max);
                None
            }
            Gesture::Listen => self.listen(Some(row)),
            Gesture::Clone => {
                self.clone_current_phrase();
                None
            }
            Gesture::Chord(d) => self.chord_step(row, d),
            Gesture::Insert | Gesture::InsertNew => match self.phrase_col {
                PhraseCol::Note => self.insert_step(row),
                PhraseCol::Chord => {
                    let chord = self.memory[self.track].chord;
                    let drum = self.track_kind() == TrackKind::Drum;
                    let step = self.project.step_mut(self.track, self.section, row)?;
                    step.root()?;
                    if !drum && step.chord == Chord::default() {
                        step.chord = chord;
                        self.modified = true;
                    }
                    self.edit_audition(row)
                }
                PhraseCol::Velocity | PhraseCol::Gate => self.listen(Some(row)),
            },
            Gesture::Edit { dir, fast } => match self.phrase_col {
                PhraseCol::Note => self.nudge_note(row, dir, fast),
                PhraseCol::Chord => match dir {
                    Dir::Left | Dir::Right => {
                        self.chord_step(row, if dir == Dir::Left { -1 } else { 1 })
                    }
                    Dir::Up | Dir::Down => {
                        if self.track_kind() == TrackKind::Drum {
                            return None;
                        }
                        let step = self.project.step_mut(self.track, self.section, row)?;
                        step.root()?;
                        let max = step.chord.max_inversion();
                        let d = if dir == Dir::Up { 1 } else { -1 };
                        step.chord.inversion =
                            (i32::from(step.chord.inversion) + d).clamp(0, max.into()) as u8;
                        let copy = *step;
                        self.modified = true;
                        self.remember(copy);
                        self.edit_audition(row)
                    }
                },
                PhraseCol::Velocity => {
                    let step = self.project.step_mut(self.track, self.section, row)?;
                    step.root()?;
                    let v = i32::from(step.velocity) + dir_delta(dir, 1, 10, fast);
                    step.velocity = v.clamp(1, 127) as u8;
                    let copy = *step;
                    self.modified = true;
                    self.remember(copy);
                    None
                }
                PhraseCol::Gate => {
                    let step = self.project.step_mut(self.track, self.section, row)?;
                    step.root()?;
                    step.gate_ticks = gate_nudge(step.gate_ticks, dir_delta(dir, 1, 4, fast));
                    let copy = *step;
                    self.modified = true;
                    self.remember(copy);
                    None
                }
            },
            Gesture::Delete => {
                match self.phrase_col {
                    PhraseCol::Note => self.delete_step(row),
                    col => {
                        if let Some(step) = self.project.step_mut(self.track, self.section, row)
                            && step.root().is_some()
                        {
                            match col {
                                PhraseCol::Chord => step.chord = Chord::default(),
                                PhraseCol::Velocity => step.velocity = DEFAULT_VELOCITY,
                                _ => step.gate_ticks = DEFAULT_GATE,
                            }
                            self.modified = true;
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn handle_project(&mut self, g: Gesture) -> Option<Audition> {
        let last = ProjectItem::ALL.len() - 1;
        match g {
            Gesture::Move(Dir::Up) => self.project_item = self.project_item.saturating_sub(1),
            Gesture::Move(Dir::Down) => self.project_item = (self.project_item + 1).min(last),
            Gesture::Jump(Dir::Up) => self.project_item = 0,
            Gesture::Jump(Dir::Down) => self.project_item = last,
            Gesture::Cancel => self.go(self.origin),
            Gesture::Insert | Gesture::InsertNew | Gesture::Edit { .. } => {
                match ProjectItem::ALL[self.project_item] {
                    ProjectItem::Font => {
                        self.font = match self.font {
                            FontSize::Small => FontSize::Large,
                            FontSize::Large => FontSize::Small,
                        }
                    }
                    ProjectItem::AuditionOnEdit => self.audition_on_edit = !self.audition_on_edit,
                    ProjectItem::Follow => self.follow = !self.follow,
                    ProjectItem::Quit => {
                        if matches!(g, Gesture::Insert) {
                            self.quit = true;
                        }
                    }
                }
            }
            Gesture::Listen => return self.listen(None),
            _ => {}
        }
        None
    }

    /// Instrumento y Mezcla (pendientes).
    fn handle_other(&mut self, g: Gesture) -> Option<Audition> {
        match g {
            Gesture::Listen => self.listen(None),
            Gesture::Section(d) => {
                self.section = if d > 0 {
                    self.project.next_section(self.section)
                } else {
                    self.project.prev_section(self.section)
                };
                None
            }
            Gesture::Cancel if self.screen == Screen::Mix => {
                self.go(self.origin);
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChordKind, GATES};

    fn app() -> App {
        App::new(Project::new_default())
    }

    fn step(app: &App, track: usize, i: usize) -> Step {
        *app.project.step(track, app.section, i).unwrap()
    }

    fn edit(dir: Dir) -> Gesture {
        Gesture::Edit { dir, fast: false }
    }

    #[test]
    fn insert_uses_track_memory_and_auditions() {
        let mut a = app();
        let aud = a.handle(Gesture::Insert, 0).unwrap();
        assert_eq!(step(&a, 0, 0).note, NoteCell::On(60));
        assert_eq!(aud.notes.as_slice(), &[60]);
        assert!(a.modified);
    }

    #[test]
    fn edit_note_fine_and_coarse() {
        let mut a = app();
        a.handle(Gesture::Insert, 0);
        a.handle(edit(Dir::Right), 0);
        a.handle(edit(Dir::Up), 0);
        assert_eq!(step(&a, 0, 0).note, NoteCell::On(73));
        a.handle(
            Gesture::Edit {
                dir: Dir::Down,
                fast: true,
            },
            0,
        );
        assert_eq!(step(&a, 0, 0).note, NoteCell::On(25));
        // La pista recuerda la última nota: el próximo paso la repite.
        a.handle(Gesture::Move(Dir::Right), 0);
        a.handle(Gesture::Insert, 0);
        assert_eq!(step(&a, 0, 1).note, NoteCell::On(25));
        // Tope inferior C-0.
        for _ in 0..5 {
            a.handle(edit(Dir::Down), 0);
        }
        assert_eq!(step(&a, 0, 1).note, NoteCell::On(NOTE_MIN));
    }

    #[test]
    fn delete_clears_step() {
        let mut a = app();
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Delete, 0);
        assert!(step(&a, 0, 0).is_empty());
    }

    #[test]
    fn cursor_moves_and_enters_fra_column() {
        let mut a = app();
        a.handle(Gesture::Move(Dir::Left), 0);
        assert_eq!(a.grid_col, GridCol::Phrase);
        a.handle(Gesture::Move(Dir::Left), 0);
        assert_eq!(a.grid_col, GridCol::Phrase);
        a.handle(Gesture::Jump(Dir::Right), 0);
        assert_eq!(a.grid_col, GridCol::Step(15));
        a.handle(Gesture::Move(Dir::Right), 0);
        assert_eq!(a.grid_col, GridCol::Step(15));
        a.handle(Gesture::Move(Dir::Up), 0);
        assert_eq!(a.track, 0);
        a.handle(Gesture::Jump(Dir::Down), 0);
        assert_eq!(a.track, 7);
        a.handle(Gesture::Track(-1), 0);
        assert_eq!(a.track, 6);
    }

    #[test]
    fn insert_in_new_section_creates_phrase() {
        let mut a = app();
        a.handle(Gesture::Section(1), 0);
        assert_eq!(a.section, Section { row: 0, entry: 1 });
        a.handle(Gesture::Insert, 10);
        assert_eq!(a.project.chain_at(0, 0).unwrap().len(), 2);
        assert_eq!(step(&a, 0, 0).note, NoteCell::On(60));
        assert_eq!(
            a.message_at(10).unwrap().text,
            "frase 008 agregada a la cadena"
        );
        assert!(a.message_at(10 + MESSAGE_MS).is_none());
    }

    #[test]
    fn gap_is_reported_and_nothing_created() {
        let mut a = app();
        // La pista 0 llega a la sección 2; la pista 1 tiene solo la 1.
        a.handle(Gesture::Section(1), 0);
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Section(1), 0);
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.section.entry, 2);
        a.handle(Gesture::Move(Dir::Down), 0);
        assert!(a.handle(Gesture::Insert, 0).is_none());
        let msg = a.message_at(0).unwrap();
        assert!(msg.warn);
        assert_eq!(msg.text, "la cadena de esta pista termina en la sección 1");
        assert_eq!(a.project.chain_at(1, 0).unwrap().len(), 1);
    }

    #[test]
    fn insert_in_new_row_creates_chain() {
        let mut a = app();
        a.handle(Gesture::Section(1), 0);
        a.handle(Gesture::Section(1), 0);
        assert_eq!(a.section, Section { row: 1, entry: 0 });
        a.handle(Gesture::Move(Dir::Down), 0);
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.project.song.rows.len(), 2);
        assert_eq!(a.project.chain_id_at(1, 1), Some(8));
        assert_eq!(
            a.message_at(0).unwrap().text,
            "cadena 008 y frase 008 creadas"
        );
    }

    #[test]
    fn fra_column_edit_remove_and_insert_last() {
        let mut a = app();
        a.grid_col = GridCol::Phrase;
        a.handle(edit(Dir::Up), 0);
        assert_eq!(a.project.phrase_id_at(0, a.section), Some(10));
        a.handle(edit(Dir::Left), 0);
        assert_eq!(a.project.phrase_id_at(0, a.section), Some(9));
        assert_eq!(a.last_phrase, Some(9));

        a.handle(Gesture::Delete, 0);
        assert_eq!(a.project.phrase_id_at(0, a.section), None);
        // A en FRA vacía: la última frase usada.
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.project.phrase_id_at(0, a.section), Some(9));
    }

    #[test]
    fn fra_double_tap_creates_new_phrase() {
        let mut a = app();
        a.grid_col = GridCol::Phrase;
        a.handle(Gesture::Delete, 0);
        a.last_phrase = Some(3);
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.project.phrase_id_at(0, a.section), Some(3));
        a.handle(Gesture::InsertNew, 100);
        assert_eq!(a.project.phrase_id_at(0, a.section), Some(8));
        assert_eq!(a.project.phrase_uses(3), 1);
    }

    #[test]
    fn clone_in_fra_column() {
        let mut a = app();
        a.handle(Gesture::Insert, 0);
        a.grid_col = GridCol::Phrase;
        a.handle(Gesture::Clone, 0);
        let id = a.project.phrase_id_at(0, a.section).unwrap();
        assert_eq!(id, 8);
        assert_eq!(step(&a, 0, 0).note, NoteCell::On(60));
        assert_eq!(a.project.phrase_uses(0), 0);
    }

    #[test]
    fn chords_on_synth_not_on_drums() {
        let mut a = app();
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Chord(1), 0);
        assert_eq!(step(&a, 0, 0).chord.kind, ChordKind::None);
        assert!(a.message_at(0).unwrap().warn);

        a.track = 7;
        a.handle(Gesture::Insert, 0);
        let aud = a.handle(Gesture::Chord(3), 0).unwrap();
        assert_eq!(step(&a, 7, 0).chord.kind, ChordKind::Major);
        assert_eq!(aud.notes.as_slice(), &[60, 64, 67]);
        // El próximo paso repite el acorde.
        a.handle(Gesture::Move(Dir::Right), 0);
        a.handle(Gesture::Insert, 0);
        assert_eq!(step(&a, 7, 1).chord.kind, ChordKind::Major);
    }

    #[test]
    fn screen_navigation() {
        let mut a = app();
        a.handle(Gesture::Screen(Dir::Left), 0);
        assert_eq!(a.screen, Screen::Song);
        a.handle(Gesture::Screen(Dir::Left), 0);
        assert_eq!(a.screen, Screen::Song);
        a.handle(Gesture::Screen(Dir::Right), 0);
        a.handle(Gesture::Screen(Dir::Right), 0);
        a.handle(Gesture::Screen(Dir::Right), 0);
        assert_eq!(a.screen, Screen::Phrase);
        a.handle(Gesture::Screen(Dir::Up), 0);
        assert_eq!(a.screen, Screen::Project);
        a.handle(Gesture::Screen(Dir::Left), 0);
        assert_eq!(a.screen, Screen::Project);
        a.handle(Gesture::Screen(Dir::Down), 0);
        assert_eq!(a.screen, Screen::Phrase);
        a.handle(Gesture::Screen(Dir::Down), 0);
        assert_eq!(a.screen, Screen::Mix);
        a.handle(Gesture::Screen(Dir::Up), 0);
        assert_eq!(a.screen, Screen::Phrase);
    }

    #[test]
    fn grid_and_phrase_share_the_step() {
        let mut a = app();
        a.grid_col = GridCol::Step(5);
        a.screen = Screen::Chain;
        a.go(Screen::Grid);
        a.go(Screen::Phrase);
        assert_eq!(a.phrase_row, 5);
        a.handle(Gesture::Move(Dir::Down), 0);
        a.go(Screen::Grid);
        assert_eq!(a.grid_col, GridCol::Step(6));
    }

    #[test]
    fn phrase_columns_edit() {
        let mut a = app();
        a.track = 7;
        a.screen = Screen::Phrase;
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Move(Dir::Right), 0);
        assert_eq!(a.phrase_col, PhraseCol::Chord);
        a.handle(edit(Dir::Right), 0);
        a.handle(edit(Dir::Up), 0);
        let s = step(&a, 7, 0);
        assert_eq!(s.chord.kind, ChordKind::Fifth);
        assert_eq!(s.chord.inversion, 1);
        a.handle(edit(Dir::Up), 0);
        assert_eq!(step(&a, 7, 0).chord.inversion, 1, "un 5 tiene 1 inversión");

        a.handle(Gesture::Move(Dir::Right), 0);
        a.handle(edit(Dir::Down), 0);
        assert_eq!(step(&a, 7, 0).velocity, 90);
        a.handle(Gesture::Delete, 0);
        assert_eq!(step(&a, 7, 0).velocity, DEFAULT_VELOCITY);

        a.handle(Gesture::Jump(Dir::Right), 0);
        assert_eq!(a.phrase_col, PhraseCol::Gate);
        a.handle(edit(Dir::Up), 0);
        assert_eq!(step(&a, 7, 0).gate_ticks, GATES[7]);
        a.handle(Gesture::Jump(Dir::Left), 0);
        a.handle(Gesture::Delete, 0);
        assert!(step(&a, 7, 0).is_empty());
    }

    #[test]
    fn phrase_screen_l2_r2_moves_within_chain() {
        let mut a = app();
        a.screen = Screen::Phrase;
        a.handle(Gesture::Section(1), 0);
        assert_eq!(a.section.entry, 1, "llega a la entrada nueva");
        a.handle(Gesture::Section(1), 0);
        assert_eq!(a.section.entry, 1);
        a.handle(Gesture::Section(-1), 0);
        assert_eq!(a.section.entry, 0);
    }

    #[test]
    fn octave_changes_new_notes() {
        let mut a = app();
        a.handle(Gesture::Octave(1), 0);
        a.handle(Gesture::Insert, 0);
        assert_eq!(step(&a, 0, 0).note, NoteCell::On(72));
        for _ in 0..20 {
            a.handle(Gesture::Octave(1), 0);
        }
        assert_eq!(a.octave, 9);
        assert!(a.memory.iter().all(|m| m.note <= NOTE_MAX));
    }

    #[test]
    fn project_screen_font_toggle_and_quit() {
        let mut a = app();
        a.handle(Gesture::Screen(Dir::Up), 0);
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.font, FontSize::Large);
        a.handle(edit(Dir::Right), 0);
        assert_eq!(a.font, FontSize::Small);
        a.handle(Gesture::Jump(Dir::Down), 0);
        a.handle(edit(Dir::Right), 0);
        assert!(!a.quit, "A + cruz no sale");
        a.handle(Gesture::Insert, 0);
        assert!(a.quit);
    }

    fn head(row: u8, entry: u8, step: u8) -> HeadPos {
        HeadPos {
            row,
            entry,
            step,
            active: true,
        }
    }

    fn playing(heads: [HeadPos; TRACKS]) -> Playhead {
        Playhead {
            playing: true,
            heads,
        }
    }

    #[test]
    fn start_is_contextual_and_select_start_loops_the_section() {
        let mut a = app();
        a.project.set_song_cell(1, 3, Some(3)).unwrap();
        a.section = Section { row: 1, entry: 0 };
        a.track = 3;
        let cases = [
            (Screen::Song, PlayMode::Song { from_row: 1 }),
            (Screen::Grid, PlayMode::Song { from_row: 1 }),
            (Screen::Chain, PlayMode::Chain { track: 3, row: 1 }),
            (
                Screen::Phrase,
                PlayMode::Phrase {
                    track: 3,
                    row: 1,
                    entry: 0,
                },
            ),
            (
                Screen::Instrument,
                PlayMode::Phrase {
                    track: 3,
                    row: 1,
                    entry: 0,
                },
            ),
            (Screen::Mix, PlayMode::Song { from_row: 1 }),
            (Screen::Project, PlayMode::Song { from_row: 1 }),
        ];
        for (screen, mode) in cases {
            a.screen = screen;
            a.handle(Gesture::Play, 0);
            assert_eq!(a.transport, Some(mode), "Start en {screen:?}");
            a.handle(Gesture::Play, 0);
            assert_eq!(a.transport, None, "con algo sonando, Start para");
            a.handle(Gesture::LoopSection, 0);
            assert_eq!(
                a.transport,
                Some(PlayMode::Section { row: 1, entry: 0 }),
                "Select + Start en {screen:?}"
            );
            a.handle(Gesture::LoopSection, 0);
            assert_eq!(a.transport, None, "Select + Start también para");
        }
    }

    #[test]
    fn playhead_shows_only_in_the_section_it_plays() {
        let mut a = app();
        a.follow = false;
        a.handle(Gesture::Section(1), 0);
        a.set_playhead(playing([head(0, 1, 5); TRACKS]));
        assert_eq!(a.playing_step(3), Some(5));
        a.handle(Gesture::Section(-1), 0);
        assert_eq!(a.playing_step(3), None, "el cabezal está en otra sección");
    }

    #[test]
    fn follow_moves_the_grid_with_the_head_of_the_current_track() {
        let mut a = app();
        assert!(a.follow, "activado por defecto");
        let mut heads = [head(0, 0, 0); TRACKS];
        a.set_playhead(playing(heads));
        assert_eq!(a.section, Section { row: 0, entry: 0 });

        // La pista 0 pasa a la sección (0, 1); la pista 7 ya está en la fila 1.
        heads[0] = head(0, 1, 0);
        heads[7] = head(1, 0, 3);
        a.set_playhead(playing(heads));
        assert_eq!(a.section, Section { row: 0, entry: 1 });
        assert_eq!(a.shown_section(7), Section { row: 1, entry: 0 });
        assert_eq!(a.playing_step(7), Some(3), "cada pista con su cabezal");

        // L2 mira otra sección hasta que el cabezal cambie de sección.
        a.handle(Gesture::Section(-1), 0);
        heads[0].step = 1;
        a.set_playhead(playing(heads));
        assert_eq!(a.section, Section { row: 0, entry: 0 });
        heads[0] = head(1, 0, 0);
        a.set_playhead(playing(heads));
        assert_eq!(a.section, Section { row: 1, entry: 0 });

        // Cambiar de pista salta a la sección de su cabezal.
        heads[2] = head(0, 0, 9);
        a.set_playhead(playing(heads));
        a.handle(Gesture::Track(2), 0);
        assert_eq!(a.section, Section { row: 0, entry: 0 });

        // En la Canción no sigue; sin la opción, tampoco.
        a.screen = Screen::Song;
        heads[2] = head(0, 1, 0);
        a.set_playhead(playing(heads));
        assert_eq!(a.section, Section { row: 0, entry: 0 });
        a.screen = Screen::Grid;
        a.follow = false;
        heads[2] = head(1, 0, 0);
        a.set_playhead(playing(heads));
        assert_eq!(a.section, Section { row: 0, entry: 0 });
    }

    #[test]
    fn follow_is_a_project_setting() {
        let mut a = app();
        a.handle(Gesture::Screen(Dir::Up), 0);
        a.project_item = 2;
        a.handle(Gesture::Insert, 0);
        assert!(!a.follow);
    }

    #[test]
    fn song_cells_insert_new_change_delete_and_clone() {
        let mut a = app();
        a.screen = Screen::Song;
        a.handle(Gesture::Move(Dir::Down), 0);
        assert_eq!(a.section.row, 1, "fila nueva");
        a.handle(Gesture::Move(Dir::Down), 0);
        assert_eq!(a.section.row, 1, "no pasa de la fila nueva");

        // Sin última cadena, A crea una; el doble toque se queda con esa (no deja huérfanas).
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.project.chain_id_at(0, 1), Some(8));
        assert_eq!(a.project.song.rows.len(), 2);
        a.handle(Gesture::InsertNew, 100);
        assert_eq!(a.project.chain_id_at(0, 1), Some(8));
        assert_eq!(a.project.free_chain(), Some(9));

        // A en la celda de al lado: la última cadena usada; doble toque: una nueva.
        a.handle(Gesture::Move(Dir::Right), 1000);
        a.handle(Gesture::Insert, 1000);
        assert_eq!(a.project.chain_id_at(1, 1), Some(8));
        a.handle(Gesture::InsertNew, 1100);
        assert_eq!(a.project.chain_id_at(1, 1), Some(9));
        assert_eq!(a.last_chain, Some(9));

        // A + cruz cambia el número (y crea la cadena si no existía).
        a.handle(edit(Dir::Up), 2000);
        assert_eq!(a.project.chain_id_at(1, 1), Some(19));
        assert!(a.project.chain(19).is_some());
        a.handle(edit(Dir::Left), 2000);
        assert_eq!(a.project.chain_id_at(1, 1), Some(18));

        // A + Y clona la cadena 0 de la fila 0.
        a.handle(Gesture::Move(Dir::Up), 3000);
        a.handle(Gesture::Clone, 3000);
        let clone = a.project.chain_id_at(1, 0).unwrap();
        assert_ne!(clone, 1);
        assert_eq!(a.project.chain(clone), a.project.chain(1));

        // A + B vacía la celda.
        a.handle(Gesture::Delete, 4000);
        assert_eq!(a.project.chain_id_at(1, 0), None);
        assert!(a.modified);

        // B + cruz y L2/R2 saltan de a 16 filas (sin pasar de la fila nueva).
        a.handle(Gesture::Jump(Dir::Down), 5000);
        assert_eq!(a.section.row, 2);
        a.handle(Gesture::Section(-1), 5000);
        assert_eq!(a.section.row, 0);
        a.handle(Gesture::Jump(Dir::Right), 5000);
        assert_eq!(a.track, TRACKS - 1);
    }

    #[test]
    fn chain_screen_edits_phrases_and_transpose() {
        let mut a = app();
        a.screen = Screen::Chain;
        a.track = 6;
        // Posición 2 (vacía): A pone la última frase usada (ninguna: una nueva).
        a.handle(Gesture::Move(Dir::Down), 0);
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.project.chain_at(6, 0).unwrap().len(), 2);
        let first_new = a.project.phrase_id_at(6, a.section).unwrap();
        a.handle(Gesture::InsertNew, 100);
        assert_eq!(
            a.project.phrase_id_at(6, a.section),
            Some(first_new),
            "el doble toque no deja una frase huérfana"
        );
        // Hueco: la posición 4 con la cadena de largo 2.
        a.handle(Gesture::Jump(Dir::Up), 1000);
        for _ in 0..3 {
            a.handle(Gesture::Move(Dir::Down), 1000);
        }
        a.handle(Gesture::Insert, 1000);
        assert!(a.message_at(1000).unwrap().warn);
        assert_eq!(a.project.chain_at(6, 0).unwrap().len(), 2);

        // Transposición de la posición 2.
        a.handle(Gesture::Move(Dir::Up), 2000);
        a.handle(Gesture::Move(Dir::Up), 2000);
        a.handle(Gesture::Move(Dir::Right), 2000);
        assert_eq!(a.chain_col, ChainCol::Transpose);
        a.handle(edit(Dir::Up), 2000);
        a.handle(edit(Dir::Right), 2000);
        let entry = |a: &App| a.project.chain_at(6, 0).unwrap().entry(1).unwrap();
        assert_eq!(entry(&a).transpose, 13);
        for _ in 0..10 {
            a.handle(edit(Dir::Up), 2000);
        }
        assert_eq!(entry(&a).transpose, TRANSPOSE_MAX);
        a.handle(Gesture::Delete, 2000);
        assert_eq!(entry(&a).transpose, 0);

        // A + B en FRASE quita la entrada; A + Y clona la frase.
        a.handle(Gesture::Move(Dir::Left), 3000);
        a.handle(Gesture::Clone, 3000);
        assert_ne!(entry(&a).phrase, first_new);
        a.handle(Gesture::Delete, 3000);
        assert_eq!(a.project.chain_at(6, 0).unwrap().len(), 1);

        // L2/R2 cambian de fila; en una fila sin cadena, A la crea.
        a.handle(Gesture::Section(1), 4000);
        assert_eq!(a.section, Section { row: 1, entry: 0 });
        a.handle(Gesture::Insert, 4000);
        assert!(a.project.chain_id_at(6, 1).is_some());
        assert_eq!(a.last_chain, a.project.chain_id_at(6, 1));
    }

    #[test]
    fn transposed_audition_matches_what_the_engine_plays() {
        let mut a = app();
        a.track = 6;
        a.handle(Gesture::Insert, 0);
        a.project.set_entry_transpose(6, a.section, 5).unwrap();
        let aud = a.handle(Gesture::Listen, 0).unwrap();
        assert_eq!(aud.notes.as_slice(), &[65]);
        a.screen = Screen::Song;
        let aud = a.handle(Gesture::Listen, 0).unwrap();
        assert_eq!(
            aud.notes.as_slice(),
            &[65],
            "X en la Canción: la primera nota"
        );
    }

    #[test]
    fn navigation_carries_the_context() {
        let mut a = app();
        a.project
            .ensure_phrase(2, Section { row: 0, entry: 1 }, None)
            .unwrap();
        a.project.set_song_cell(1, 2, Some(2)).unwrap();
        // Canción: cursor en la fila 1, pista 3 → Grilla en la sección (1, 0).
        a.screen = Screen::Song;
        a.section = Section { row: 0, entry: 1 };
        a.handle(Gesture::Move(Dir::Down), 0);
        a.track = 2;
        a.handle(Gesture::Screen(Dir::Right), 0);
        assert_eq!(a.screen, Screen::Grid);
        assert_eq!(a.section, Section { row: 1, entry: 0 });
        assert_eq!(a.track, 2);
        // Grilla → Cadena con el cursor en k; Cadena → Frase con la posición del cursor.
        a.handle(Gesture::Screen(Dir::Left), 0);
        a.handle(Gesture::Move(Dir::Up), 0);
        a.handle(Gesture::Screen(Dir::Right), 0);
        a.handle(Gesture::Section(1), 0);
        assert_eq!(a.section, Section { row: 0, entry: 1 });
        a.handle(Gesture::Screen(Dir::Right), 0);
        assert_eq!(a.screen, Screen::Chain);
        a.handle(Gesture::Move(Dir::Up), 0);
        a.handle(Gesture::Screen(Dir::Right), 0);
        assert_eq!(a.screen, Screen::Phrase);
        assert_eq!(a.project.phrase_id_at(2, a.section), Some(2));
    }

    #[test]
    fn mute_tempo_and_quit_are_global() {
        let mut a = app();
        a.handle(Gesture::Mute, 0);
        assert!(a.project.tracks[0].mix.mute);
        a.handle(Gesture::Tempo(1), 0);
        assert_eq!(a.project.tempo_bpm, 121.0);
        a.handle(Gesture::Quit, 0);
        assert!(a.quit);
    }

    #[test]
    fn listen_empty_step_plays_instrument() {
        let mut a = app();
        let aud = a.handle(Gesture::Listen, 0).unwrap();
        assert_eq!(aud.notes.as_slice(), &[60]);
        assert!(step(&a, 0, 0).is_empty());
    }
}
