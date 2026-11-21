//! Estado de la interfaz y edición del proyecto a partir de gestos. Sin SDL ni audio:
//! `handle` devuelve, como mucho, una audición para mandarle al motor. Guardar y abrir
//! proyectos (H6) escribe y lee la carpeta de datos (`storage`) desde este hilo.

use crate::engine::{HeadPos, LoadReport, PlayMode, Playhead};
use crate::gestures::{Dir, Gesture};
use crate::instrument::{MAX_VOICES, default_voices};
use crate::modal::{BROWSER_PAGE, Browser, Confirm, Modal, NameEditor, NamePurpose, Then};
use crate::model::{
    CHAIN_LEN, ChainId, Chord, DEFAULT_GATE, DEFAULT_TEMPO, DEFAULT_VELOCITY, EditError,
    MAX_SONG_ROWS, NOTE_MAX, NOTE_MIN, NoteCell, NoteSet, PHRASE_STEPS, PhraseId, Project,
    SWING_MAX, SWING_MIN, Section, Step, TEMPO_MAX, TEMPO_MIN, TRACKS, TRANSPOSE_MAX, TrackKind,
    gate_nudge,
};
use crate::selection::{Clip, Selection};
use crate::storage::{
    AppConfig, DataDir, FontChoice, LoadError, PROJECT_FILE, PROJECTS_DIR, valid_name,
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

    pub fn index(self) -> usize {
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
    Name,
    Tempo,
    Swing,
    Save,
    SaveAs,
    Load,
    New,
    Font,
    AuditionOnEdit,
    Follow,
    Quit,
}

impl ProjectItem {
    pub const ALL: [ProjectItem; 11] = [
        ProjectItem::Name,
        ProjectItem::Tempo,
        ProjectItem::Swing,
        ProjectItem::Save,
        ProjectItem::SaveAs,
        ProjectItem::Load,
        ProjectItem::New,
        ProjectItem::Font,
        ProjectItem::AuditionOnEdit,
        ProjectItem::Follow,
        ProjectItem::Quit,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&i| i == self).unwrap_or(0)
    }

    /// Acciones (se ejecutan con A; A + cruz no hace nada).
    pub fn is_action(self) -> bool {
        matches!(
            self,
            ProjectItem::Name
                | ProjectItem::Save
                | ProjectItem::SaveAs
                | ProjectItem::Load
                | ProjectItem::New
                | ProjectItem::Quit
        )
    }
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
    /// Con una frase, se escucha entera una vez (X en la Canción y la Cadena) y `notes` es su
    /// primera nota.
    pub phrase: Option<PhrasePreview>,
    pub notes: NoteSet,
    pub velocity: u8,
    /// Largo en ticks (`GATE_INF` = hasta la próxima nota); la percusión lo ignora.
    pub gate: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhrasePreview {
    pub phrase: PhraseId,
    pub transpose: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub text: String,
    pub at_ms: u64,
    pub warn: bool,
}

pub const MESSAGE_MS: u64 = 3000;
const DEFAULT_OCTAVE: u8 = 4;

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
    /// El archivo abierto es de un formato más nuevo: GUARDAR no lo sobrescribe.
    pub read_only: bool,
    /// Carpeta de datos (None en los tests que no usan archivos).
    pub data: Option<DataDir>,
    pub config: AppConfig,
    /// Navegador, editor de nombres o diálogo abierto: recibe todos los gestos.
    pub modal: Option<Modal>,
    /// Modo selección (H10): mientras está activo, la cruz extiende en lugar de mover.
    pub selection: Option<Selection>,
    /// Portapapeles único: recuerda el tipo de lo copiado (spec-ui, "Modo selección").
    pub clipboard: Option<Clip>,
    pub message: Option<Message>,
    pub quit: bool,
    now: u64,
    /// El último gesto fue un A que puso un número de frase o de cadena (para el doble toque).
    just_inserted: bool,
    /// El último gesto copió con A: el segundo toque de un doble toque no inserta nada.
    pub(crate) just_copied: bool,
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
            project_item: ProjectItem::Save.index(),
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
            read_only: false,
            data: None,
            config: AppConfig::default(),
            modal: None,
            selection: None,
            clipboard: None,
            message: None,
            quit: false,
            now: 0,
            just_inserted: false,
            just_copied: false,
        }
    }

    /// Mensaje vigente (dura `MESSAGE_MS`).
    pub fn message_at(&self, now: u64) -> Option<&Message> {
        self.message
            .as_ref()
            .filter(|m| now.saturating_sub(m.at_ms) < MESSAGE_MS)
    }

    /// Aviso que viene de afuera de los gestos (por ejemplo, del sistema de audio).
    pub fn notify(&mut self, text: impl Into<String>, warn: bool, now: u64) {
        self.now = now;
        if warn {
            self.warn(text);
        } else {
            self.info(text);
        }
    }

    pub(crate) fn info(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            at_ms: self.now,
            warn: false,
        });
    }

    pub(crate) fn warn(&mut self, text: impl Into<String>) {
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

    /// La Grilla y la Frase siguen a los cabezales. Con una selección abierta la vista se queda
    /// quieta: lo que se copia es lo que se ve.
    fn following(&self) -> bool {
        self.follow
            && self.playhead.playing
            && self.selection.is_none()
            && matches!(self.screen, Screen::Grid | Screen::Phrase)
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
        if std::mem::take(&mut self.just_copied) && g == Gesture::InsertNew {
            return None;
        }
        let track = self.track;
        let screen = self.screen;
        let audition = self.dispatch(g, just_inserted);
        if self.track != track || self.screen != screen {
            self.snap_to_head();
        }
        audition
    }

    fn dispatch(&mut self, g: Gesture, just_inserted: bool) -> Option<Audition> {
        if let Some(modal) = self.modal.take() {
            self.handle_modal(modal, g);
            return None;
        }
        if self.selection.is_some() && self.handle_selection(g) {
            return None;
        }
        match g {
            Gesture::Screen(dir) => {
                self.navigate(dir);
                None
            }
            Gesture::Quit => {
                self.request_quit();
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
                self.set_tempo(self.project.tempo_bpm + d as f32);
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
            Gesture::SelectMode => {
                self.start_selection();
                None
            }
            Gesture::Paste => {
                self.paste();
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

    /// X en la Canción y la Cadena: la frase de la sección entera, una vez, con la transposición
    /// de su entrada. Sin frase o sin notas, el instrumento.
    fn listen_phrase(&self, sec: Section) -> Option<Audition> {
        let first = self
            .project
            .phrase_at(self.track, sec)
            .and_then(|p| p.steps.iter().position(|s| s.root().is_some()));
        let mut audition = self.listen_at(sec, first)?;
        if first.is_some()
            && let Some(id) = self.project.phrase_id_at(self.track, sec)
        {
            let transpose = self
                .project
                .chain_at(self.track, sec.row)
                .and_then(|c| c.entry(sec.entry))
                .map_or(0, |e| e.transpose);
            audition.phrase = Some(PhrasePreview {
                phrase: id,
                transpose,
            });
        }
        Some(audition)
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
                phrase: None,
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
            phrase: None,
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
            Gesture::Insert if ProjectItem::ALL[self.project_item].is_action() => {
                self.project_action(ProjectItem::ALL[self.project_item]);
            }
            Gesture::Edit { dir, fast } if self.project_item_is_value() => {
                match ProjectItem::ALL[self.project_item] {
                    // ±0,1 / ±1 BPM, en décimas exactas (sin acumular error de f32).
                    ProjectItem::Tempo => {
                        let tenths = (self.project.tempo_bpm * 10.0).round() as i32;
                        let tenths = tenths + dir_delta(dir, 1, 10, fast);
                        self.set_tempo(tenths as f32 / 10.0);
                    }
                    _ => {
                        let swing = i32::from(self.project.swing) + dir_delta(dir, 1, 5, fast);
                        self.set_swing(swing);
                    }
                }
            }
            Gesture::Delete if self.project_item_is_value() => {
                match ProjectItem::ALL[self.project_item] {
                    ProjectItem::Tempo => self.set_tempo(DEFAULT_TEMPO),
                    _ => self.set_swing(i32::from(SWING_MIN)),
                }
            }
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
                    // Las acciones van solo con un toque de A (un doble toque no las repite).
                    _ => return None,
                }
                self.store_settings();
            }
            Gesture::Listen => return self.listen(None),
            _ => {}
        }
        None
    }

    /// Instrumento (por ahora solo "voces", H4) y Mezcla (pendiente).
    fn handle_other(&mut self, g: Gesture) -> Option<Audition> {
        match g {
            Gesture::Edit { dir, .. } if self.screen == Screen::Instrument => {
                let voices = i32::from(self.project.tracks[self.track].instrument.voices);
                self.set_voices(voices + dir_delta(dir, 1, 1, false));
                None
            }
            Gesture::Delete if self.screen == Screen::Instrument => {
                let kind = self.track_kind();
                self.set_voices(i32::from(default_voices(self.track, kind)));
                None
            }
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

/// Archivos (H6): guardar, abrir, nuevo, renombrar y salir con cambios sin guardar.
impl App {
    /// Conecta la carpeta de datos y aplica los ajustes de `config.ron`.
    pub fn attach_data(&mut self, data: DataDir, config: AppConfig) {
        self.font = match config.ui.font {
            FontChoice::Small8x16 => FontSize::Small,
            FontChoice::Large12x24 => FontSize::Large,
        };
        self.audition_on_edit = config.ui.audition_on_edit;
        self.follow = config.ui.follow_playback;
        self.config = config;
        self.data = Some(data);
    }

    /// Al arrancar: el último proyecto usado o uno nuevo (spec-producto, F7).
    pub fn open_last_or_new(&mut self) {
        self.new_project();
        if let Some(name) = self.config.last_project.clone() {
            self.open(&name, false);
        }
    }

    /// Nombre a mostrar del proyecto con la marca de cambios sin guardar.
    pub fn project_label(&self) -> String {
        format!(
            "{}{}",
            self.project.name,
            if self.modified { "*" } else { "" }
        )
    }

    /// Está abierto el diálogo "Hay cambios sin guardar... ¿Salir igual?".
    pub fn confirming_quit(&self) -> bool {
        matches!(
            self.modal,
            Some(Modal::Confirm(Confirm::Unsaved(Then::Quit)))
        )
    }

    pub fn request_quit(&mut self) {
        if self.modified {
            self.modal = Some(Modal::Confirm(Confirm::Unsaved(Then::Quit)));
        } else {
            self.quit = true;
        }
    }

    /// Tempo y swing: valores del proyecto que se editan con A + cruz.
    fn project_item_is_value(&self) -> bool {
        matches!(
            ProjectItem::ALL[self.project_item],
            ProjectItem::Tempo | ProjectItem::Swing
        )
    }

    fn set_tempo(&mut self, bpm: f32) {
        let bpm = bpm.clamp(TEMPO_MIN, TEMPO_MAX);
        if bpm != self.project.tempo_bpm {
            self.project.tempo_bpm = bpm;
            self.modified = true;
        }
    }

    fn set_swing(&mut self, swing: i32) {
        let swing = swing.clamp(i32::from(SWING_MIN), i32::from(SWING_MAX)) as u8;
        if swing != self.project.swing {
            self.project.swing = swing;
            self.modified = true;
        }
    }

    /// Voces de la pista actual (1–8). Las notas que ya suenan siguen; el límite nuevo se aplica
    /// desde la próxima nota de la pista.
    fn set_voices(&mut self, voices: i32) {
        let voices = voices.clamp(1, i32::from(MAX_VOICES)) as u8;
        let instrument = &mut self.project.tracks[self.track].instrument;
        if voices != instrument.voices {
            instrument.voices = voices;
            self.modified = true;
        }
    }

    fn project_action(&mut self, item: ProjectItem) {
        match item {
            ProjectItem::Name => {
                self.modal = Some(Modal::Name(NameEditor::new(
                    NamePurpose::Rename,
                    &self.project.name,
                )));
            }
            ProjectItem::Save => {
                self.save();
            }
            ProjectItem::SaveAs => {
                self.modal = Some(Modal::Name(NameEditor::new(
                    NamePurpose::SaveAs,
                    &self.project.name,
                )));
            }
            ProjectItem::Load => self.open_browser(),
            ProjectItem::New => self.unless_unsaved(Then::New),
            ProjectItem::Quit => self.request_quit(),
            ProjectItem::Tempo
            | ProjectItem::Swing
            | ProjectItem::Font
            | ProjectItem::AuditionOnEdit
            | ProjectItem::Follow => {}
        }
    }

    /// Hace `then` o, si hay cambios sin guardar, pregunta primero (spec-ui 10).
    fn unless_unsaved(&mut self, then: Then) {
        if self.modified {
            self.modal = Some(Modal::Confirm(Confirm::Unsaved(then)));
        } else {
            self.run(then);
        }
    }

    fn run(&mut self, then: Then) {
        match then {
            Then::Quit => self.quit = true,
            Then::New => {
                self.new_project();
                self.info(format!("proyecto nuevo: {}", self.project.name));
            }
            Then::Load(name) => self.open(&name, false),
        }
    }

    fn open_browser(&mut self) {
        let Some(data) = &self.data else {
            self.warn("no hay carpeta de datos");
            return;
        };
        match data.list() {
            Ok(entries) => {
                self.modal = Some(Modal::Browser(Browser::new(entries, &self.project.name)));
            }
            Err(e) => self.warn(e),
        }
    }

    /// Cambia el proyecto en memoria y deja la vista en el principio. Para el transporte: el
    /// motor recibe el proyecto nuevo por diferencias (`EngineMirror`).
    fn replace_project(&mut self, project: Project, read_only: bool) {
        self.project = project;
        self.read_only = read_only;
        self.modified = false;
        self.selection = None;
        self.transport = None;
        self.section = Section::default();
        self.grid_col = GridCol::Step(0);
        self.phrase_row = 0;
        self.last_phrase = None;
        self.last_chain = None;
    }

    fn new_project(&mut self) {
        let mut project = Project::new_default();
        if let Some(data) = &self.data {
            project.name = data.free_name();
        }
        self.replace_project(project, false);
    }

    /// Abre `proyectos/<name>/`. Si `proyecto.ron` no se puede leer y hay `.bak`, lo ofrece.
    fn open(&mut self, name: &str, from_backup: bool) {
        let Some(data) = &self.data else {
            self.warn("no hay carpeta de datos");
            return;
        };
        match data.load(name, from_backup) {
            Ok(loaded) => {
                self.replace_project(loaded.project, loaded.newer);
                self.remember_last_project();
                let mut notes = Vec::new();
                if from_backup {
                    // Así GUARDAR rehace proyecto.ron con lo que se recuperó.
                    self.modified = true;
                    notes.push(
                        "abierto desde la copia .bak: GUARDAR repara proyecto.ron".to_string(),
                    );
                }
                if loaded.newer {
                    notes.push(
                        "formato más nuevo que esta app: solo lectura (usá GUARDAR COMO)"
                            .to_string(),
                    );
                }
                notes.extend(loaded.warnings);
                if notes.is_empty() {
                    self.info(format!("proyecto {name} abierto"));
                } else {
                    self.warn(format!("{name}: {}", notes.join("; ")));
                }
            }
            Err(LoadError::Missing) => self.warn(format!("no existe el proyecto {name}")),
            Err(LoadError::Broken {
                reason,
                backup: true,
            }) => {
                self.modal = Some(Modal::Confirm(Confirm::OpenBackup {
                    name: name.to_string(),
                    reason,
                }));
            }
            Err(e) => self.warn(format!("no pude abrir {name}: {e}")),
        }
    }

    /// GUARDAR. Devuelve si se guardó.
    pub fn save(&mut self) -> bool {
        let Some(data) = &self.data else {
            self.warn("no hay carpeta de datos");
            return false;
        };
        if self.read_only {
            self.warn("archivo de un formato más nuevo: guardalo con GUARDAR COMO y otro nombre");
            return false;
        }
        match data.save(&self.project) {
            Ok(_) => {
                self.modified = false;
                let text = format!(
                    "guardado en {PROJECTS_DIR}/{}/{PROJECT_FILE}",
                    self.project.name
                );
                self.remember_last_project();
                self.info(text);
                true
            }
            Err(e) => {
                self.warn(e);
                false
            }
        }
    }

    fn save_as(&mut self, name: String, overwrite: bool) {
        let Some(data) = &self.data else {
            self.warn("no hay carpeta de datos");
            return;
        };
        if name == self.project.name {
            self.save();
            return;
        }
        if data.exists(&name) && !overwrite {
            self.modal = Some(Modal::Confirm(Confirm::Overwrite { name }));
            return;
        }
        let old = std::mem::replace(&mut self.project.name, name);
        let read_only = std::mem::replace(&mut self.read_only, false);
        if !self.save() {
            self.project.name = old;
            self.read_only = read_only;
        }
    }

    fn rename(&mut self, name: String) {
        let Some(data) = &self.data else {
            self.warn("no hay carpeta de datos");
            return;
        };
        if name == self.project.name {
            return;
        }
        match data.rename(&self.project.name, &name) {
            Ok(()) => {
                if self.config.last_project.as_deref() == Some(self.project.name.as_str()) {
                    self.config.last_project = Some(name.clone());
                    self.store_config();
                }
                self.info(format!("{} ahora se llama {name}", self.project.name));
                self.project.name = name;
            }
            Err(e) => self.warn(e),
        }
    }

    fn remember_last_project(&mut self) {
        if self.config.last_project.as_deref() != Some(self.project.name.as_str()) {
            self.config.last_project = Some(self.project.name.clone());
            self.store_config();
        }
    }

    fn store_settings(&mut self) {
        self.config.ui.font = match self.font {
            FontSize::Small => FontChoice::Small8x16,
            FontSize::Large => FontChoice::Large12x24,
        };
        self.config.ui.audition_on_edit = self.audition_on_edit;
        self.config.ui.follow_playback = self.follow;
        self.store_config();
    }

    fn store_config(&mut self) {
        if let Some(data) = &self.data
            && let Err(e) = data.save_config(&self.config)
        {
            self.warn(e);
        }
    }

    /// Los modales reciben todos los gestos: Select + cruz, los gatillos y Start (salvo para
    /// aceptar) no hacen nada mientras están abiertos (spec-ui, "Modos").
    fn handle_modal(&mut self, modal: Modal, g: Gesture) {
        if g == Gesture::Quit {
            self.modal = Some(modal);
            if !self.confirming_quit() {
                self.modal = None;
                self.request_quit();
            }
            return;
        }
        let keep = match modal {
            Modal::Name(editor) => self.handle_name_editor(editor, g),
            Modal::Browser(browser) => self.handle_browser(browser, g),
            Modal::Confirm(confirm) => self.handle_confirm(confirm, g),
        };
        if keep.is_some() {
            // Un modal que abre otro (por ejemplo, Reemplazar) gana sobre el que cierra.
            self.modal = self.modal.take().or(keep);
        }
    }

    fn handle_name_editor(&mut self, mut e: NameEditor, g: Gesture) -> Option<Modal> {
        match g {
            Gesture::Move(Dir::Left) | Gesture::Edit { dir: Dir::Left, .. } => e.move_by(-1),
            Gesture::Move(Dir::Right)
            | Gesture::Edit {
                dir: Dir::Right, ..
            } => e.move_by(1),
            Gesture::Move(Dir::Up) | Gesture::Edit { dir: Dir::Up, .. } => e.cycle(1),
            Gesture::Move(Dir::Down) | Gesture::Edit { dir: Dir::Down, .. } => e.cycle(-1),
            Gesture::Jump(Dir::Left) => e.jump(false),
            Gesture::Jump(Dir::Right) => e.jump(true),
            Gesture::Delete => e.delete(),
            Gesture::Cancel => return None,
            Gesture::Play => {
                let name = e.name();
                if !valid_name(&name) {
                    self.warn("el nombre no puede estar vacío");
                } else {
                    match e.purpose {
                        NamePurpose::SaveAs => self.save_as(name, false),
                        NamePurpose::Rename => self.rename(name),
                    }
                    return None;
                }
            }
            _ => {}
        }
        Some(Modal::Name(e))
    }

    fn handle_browser(&mut self, mut b: Browser, g: Gesture) -> Option<Modal> {
        match g {
            Gesture::Move(Dir::Up) => b.move_by(-1),
            Gesture::Move(Dir::Down) => b.move_by(1),
            Gesture::Jump(Dir::Up) => b.move_by(-(BROWSER_PAGE as i32)),
            Gesture::Jump(Dir::Down) => b.move_by(BROWSER_PAGE as i32),
            Gesture::Insert | Gesture::Move(Dir::Right) => {
                if let Some(entry) = b.selected() {
                    let name = entry.name.clone();
                    self.unless_unsaved(Then::Load(name));
                    return None;
                }
            }
            Gesture::Cancel => return None,
            _ => {}
        }
        Some(Modal::Browser(b))
    }

    fn handle_confirm(&mut self, c: Confirm, g: Gesture) -> Option<Modal> {
        match (g, c) {
            (Gesture::Cancel, _) => None,
            (Gesture::Play, Confirm::Unsaved(then)) => {
                if self.save() {
                    self.run(then);
                }
                None
            }
            (Gesture::Insert, Confirm::Unsaved(then)) => {
                self.run(then);
                None
            }
            (Gesture::Insert, Confirm::OpenBackup { name, .. }) => {
                self.open(&name, true);
                None
            }
            (Gesture::Insert, Confirm::Overwrite { name }) => {
                self.save_as(name, true);
                None
            }
            (_, c) => Some(Modal::Confirm(c)),
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
        assert_eq!(ProjectItem::ALL[a.project_item], ProjectItem::Save);
        a.project_item = ProjectItem::Font.index();
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
        a.project_item = ProjectItem::Follow.index();
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
        assert_eq!(aud.phrase, None, "X en la Grilla: el paso");
        let phrase = a.project.phrase_id_at(6, a.section).unwrap();
        let expected = Some(PhrasePreview {
            phrase,
            transpose: 5,
        });
        for screen in [Screen::Song, Screen::Chain] {
            a.screen = screen;
            let aud = a.handle(Gesture::Listen, 0).unwrap();
            assert_eq!(aud.phrase, expected, "{screen:?}: la frase entera, una vez");
            assert_eq!(aud.notes.as_slice(), &[65]);
        }
        // Una frase sin notas: el instrumento.
        a.project.step_mut(6, a.section, 0).unwrap().note = NoteCell::Empty;
        assert_eq!(a.handle(Gesture::Listen, 0).unwrap().phrase, None);
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
        assert!(!a.quit, "con cambios sin guardar, pregunta");
        assert!(a.confirming_quit());
        a.handle(Gesture::Insert, 0);
        assert!(a.quit, "A: salir sin guardar");
    }

    #[test]
    fn tempo_and_swing_are_edited_in_project() {
        let mut a = app();
        a.handle(Gesture::Screen(Dir::Up), 0);
        assert_eq!(a.screen, Screen::Project);
        a.handle(Gesture::Move(Dir::Up), 0);
        a.handle(Gesture::Move(Dir::Up), 0);
        assert_eq!(ProjectItem::ALL[a.project_item], ProjectItem::Tempo);
        a.handle(Gesture::Insert, 0);
        assert!(!a.modified, "A solo no cambia un valor");
        for _ in 0..3 {
            a.handle(edit(Dir::Right), 0);
        }
        assert_eq!(a.project.tempo_bpm, 120.3);
        assert!(a.modified);
        a.handle(edit(Dir::Up), 0);
        a.handle(
            Gesture::Edit {
                dir: Dir::Down,
                fast: true,
            },
            0,
        );
        assert_eq!(a.project.tempo_bpm, 117.3);
        a.handle(Gesture::Delete, 0);
        assert_eq!(a.project.tempo_bpm, 120.0, "A + B: valor por defecto");
        // Mantenido, ±4 BPM por repetición.
        for _ in 0..50 {
            a.handle(
                Gesture::Edit {
                    dir: Dir::Up,
                    fast: true,
                },
                0,
            );
        }
        assert_eq!(a.project.tempo_bpm, TEMPO_MAX);

        a.handle(Gesture::Move(Dir::Down), 0);
        assert_eq!(ProjectItem::ALL[a.project_item], ProjectItem::Swing);
        a.handle(edit(Dir::Up), 0);
        a.handle(edit(Dir::Right), 0);
        assert_eq!(a.project.swing, 56);
        for _ in 0..10 {
            a.handle(edit(Dir::Up), 0);
        }
        assert_eq!(a.project.swing, SWING_MAX);
        a.handle(Gesture::Delete, 0);
        assert_eq!(a.project.swing, SWING_MIN);
        a.handle(edit(Dir::Left), 0);
        assert_eq!(a.project.swing, SWING_MIN);
    }

    #[test]
    fn voices_are_edited_in_instrument() {
        let mut a = app();
        a.track = 7;
        a.screen = Screen::Instrument;
        assert_eq!(a.project.tracks[7].instrument.voices, 4);
        a.handle(edit(Dir::Left), 0);
        a.handle(edit(Dir::Down), 0);
        assert_eq!(a.project.tracks[7].instrument.voices, 2);
        assert!(a.modified);
        for _ in 0..10 {
            a.handle(edit(Dir::Up), 0);
        }
        assert_eq!(a.project.tracks[7].instrument.voices, MAX_VOICES);
        a.handle(Gesture::Delete, 0);
        assert_eq!(
            a.project.tracks[7].instrument.voices, 4,
            "A + B: las del kit"
        );
        a.handle(Gesture::Track(-1), 0);
        assert_eq!(a.track, 6);
        a.handle(Gesture::Delete, 0);
        assert_eq!(a.project.tracks[6].instrument.voices, 1, "el bajo es mono");
        for _ in 0..3 {
            a.handle(edit(Dir::Left), 0);
        }
        assert_eq!(a.project.tracks[6].instrument.voices, 1);
    }

    #[test]
    fn listen_empty_step_plays_instrument() {
        let mut a = app();
        let aud = a.handle(Gesture::Listen, 0).unwrap();
        assert_eq!(aud.notes.as_slice(), &[60]);
        assert!(step(&a, 0, 0).is_empty());
    }

    // --- Archivos (H6) ---

    use crate::storage::test_dir::TempDir;
    use crate::storage::{BACKUP_FILE, CONFIG_FILE};

    fn app_with_data(tmp: &TempDir) -> App {
        let mut a = App::new(Project::new_default());
        let data = DataDir::new(tmp.path());
        let config = data.load_config().unwrap();
        a.attach_data(data, config);
        a.open_last_or_new();
        a
    }

    fn on_project_item(a: &mut App, item: ProjectItem) {
        if a.screen != Screen::Project {
            a.handle(Gesture::Screen(Dir::Up), 0);
        }
        a.project_item = item.index();
    }

    fn data(a: &App) -> &DataDir {
        a.data.as_ref().unwrap()
    }

    #[test]
    fn save_marks_clean_and_reopens_at_start() {
        let tmp = TempDir::new();
        let mut a = app_with_data(&tmp);
        assert_eq!(a.project.name, "proyecto-001");
        assert!(!a.modified);
        a.handle(Gesture::Insert, 0);
        assert!(a.modified);
        assert_eq!(a.project_label(), "proyecto-001*");

        on_project_item(&mut a, ProjectItem::Save);
        a.handle(Gesture::Insert, 0);
        assert!(!a.modified, "{:?}", a.message);
        assert!(
            tmp.path()
                .join("proyectos/proyecto-001/proyecto.ron")
                .is_file()
        );
        assert!(tmp.path().join(CONFIG_FILE).is_file());

        // Al arrancar de nuevo se abre el último proyecto, con la nota puesta.
        let b = app_with_data(&tmp);
        assert_eq!(b.project, a.project);
        assert_eq!(b.config.last_project.as_deref(), Some("proyecto-001"));
        assert_eq!(
            b.project.step(0, Section::default(), 0).unwrap().note,
            NoteCell::On(60)
        );
    }

    #[test]
    fn new_and_load_ask_when_there_are_unsaved_changes() {
        let tmp = TempDir::new();
        let mut a = app_with_data(&tmp);
        a.handle(Gesture::Insert, 0);
        on_project_item(&mut a, ProjectItem::Save);
        a.handle(Gesture::Insert, 0);
        let saved = a.project.clone();

        // Sin cambios, NUEVO no pregunta.
        on_project_item(&mut a, ProjectItem::New);
        a.handle(Gesture::Insert, 0);
        assert!(a.modal.is_none());
        assert_eq!(a.project.name, "proyecto-002");
        a.handle(Gesture::Tempo(1), 0);

        // Con cambios: B cancela, A crea sin guardar.
        a.handle(Gesture::Insert, 0);
        assert!(matches!(
            a.modal,
            Some(Modal::Confirm(Confirm::Unsaved(Then::New)))
        ));
        a.handle(Gesture::Cancel, 0);
        assert!(a.modal.is_none());
        assert_eq!(a.project.tempo_bpm, 121.0);
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.project.tempo_bpm, 120.0);
        assert!(!data(&a).exists("proyecto-002"), "no se guardó");

        // CARGAR: el navegador lista lo guardado; con cambios, Start guarda y abre.
        a.handle(Gesture::Tempo(-1), 0);
        on_project_item(&mut a, ProjectItem::Load);
        a.handle(Gesture::Insert, 0);
        let Some(Modal::Browser(b)) = &a.modal else {
            panic!("tendría que abrir el navegador");
        };
        assert_eq!(b.entries.len(), 1);
        // Los gestos globales no hacen nada con un modal abierto.
        a.handle(Gesture::Screen(Dir::Down), 0);
        a.handle(Gesture::Play, 0);
        assert_eq!(a.screen, Screen::Project);
        assert!(a.transport.is_none());
        a.handle(Gesture::Insert, 0);
        assert!(matches!(
            a.modal,
            Some(Modal::Confirm(Confirm::Unsaved(Then::Load(_))))
        ));
        a.handle(Gesture::Play, 0);
        assert!(a.modal.is_none());
        assert_eq!(a.project, saved);
        assert!(!a.modified);
        assert_eq!(
            data(&a)
                .load("proyecto-002", false)
                .unwrap()
                .project
                .tempo_bpm,
            119.0
        );
    }

    #[test]
    fn save_as_types_a_name_and_asks_before_replacing() {
        let tmp = TempDir::new();
        let mut a = app_with_data(&tmp);
        on_project_item(&mut a, ProjectItem::SaveAs);
        a.handle(Gesture::Insert, 0);
        // Borra "proyecto-001" (12 letras) y escribe "b".
        for _ in 0..12 {
            a.handle(Gesture::Delete, 0);
        }
        a.handle(Gesture::Play, 0);
        assert!(matches!(a.modal, Some(Modal::Name(_))), "vacío: no acepta");
        a.handle(edit(Dir::Up), 0);
        a.handle(edit(Dir::Up), 0);
        a.handle(Gesture::Play, 0);
        assert!(a.modal.is_none());
        assert_eq!(a.project.name, "b");
        assert!(data(&a).exists("b"));
        assert_eq!(a.config.last_project.as_deref(), Some("b"));

        // Guardar como "b" otro proyecto: pregunta y, con A, reemplaza (lo anterior queda en .bak).
        a.handle(Gesture::Tempo(5), 0);
        a.project.name = "c".to_string();
        on_project_item(&mut a, ProjectItem::SaveAs);
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Delete, 0);
        a.handle(edit(Dir::Up), 0);
        a.handle(edit(Dir::Up), 0);
        a.handle(Gesture::Play, 0);
        assert!(matches!(
            a.modal,
            Some(Modal::Confirm(Confirm::Overwrite { .. }))
        ));
        assert_eq!(a.project.name, "c");
        a.handle(Gesture::Insert, 0);
        assert_eq!(a.project.name, "b");
        assert_eq!(data(&a).load("b", false).unwrap().project.tempo_bpm, 125.0);
        assert!(tmp.path().join("proyectos/b").join(BACKUP_FILE).is_file());
    }

    #[test]
    fn rename_moves_the_folder() {
        let tmp = TempDir::new();
        let mut a = app_with_data(&tmp);
        on_project_item(&mut a, ProjectItem::Save);
        a.handle(Gesture::Insert, 0);
        on_project_item(&mut a, ProjectItem::Name);
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Move(Dir::Up), 0);
        a.handle(Gesture::Play, 0);
        assert_eq!(a.project.name, "proyecto-001a");
        assert!(data(&a).exists("proyecto-001a"));
        assert!(!data(&a).exists("proyecto-001"));
        assert_eq!(a.config.last_project.as_deref(), Some("proyecto-001a"));
        assert!(!a.modified);
    }

    #[test]
    fn unreadable_project_offers_the_backup_at_start() {
        let tmp = TempDir::new();
        let mut a = app_with_data(&tmp);
        on_project_item(&mut a, ProjectItem::Save);
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Tempo(10), 0);
        a.handle(Gesture::Insert, 0);
        let main = tmp.path().join("proyectos/proyecto-001/proyecto.ron");
        std::fs::write(&main, "(format_version: 1, tempo_bpm: ").unwrap();

        let mut b = app_with_data(&tmp);
        assert!(matches!(
            b.modal,
            Some(Modal::Confirm(Confirm::OpenBackup { .. }))
        ));
        assert_eq!(b.project.name, "proyecto-002", "mientras tanto, uno nuevo");
        b.handle(Gesture::Insert, 0);
        assert!(b.modal.is_none());
        assert_eq!(b.project.name, "proyecto-001");
        assert_eq!(b.project.tempo_bpm, 120.0, "la versión del .bak");
        assert!(b.modified, "GUARDAR repara proyecto.ron");
        assert!(b.message.as_ref().unwrap().warn);
        b.handle(Gesture::Screen(Dir::Up), 0);
        b.project_item = ProjectItem::Save.index();
        b.handle(Gesture::Insert, 0);
        assert!(data(&b).load("proyecto-001", false).is_ok());
    }

    #[test]
    fn newer_format_is_read_only() {
        let tmp = TempDir::new();
        let dir = tmp.path().join("proyectos/futuro");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(PROJECT_FILE), "(format_version: 7)").unwrap();
        std::fs::write(tmp.path().join(CONFIG_FILE), "(last_project: \"futuro\")").unwrap();
        let mut a = app_with_data(&tmp);
        assert_eq!(a.project.name, "futuro");
        assert!(a.read_only);
        on_project_item(&mut a, ProjectItem::Save);
        a.handle(Gesture::Insert, 0);
        assert!(a.message.as_ref().unwrap().warn);
        assert_eq!(
            std::fs::read_to_string(dir.join(PROJECT_FILE)).unwrap(),
            "(format_version: 7)"
        );
        on_project_item(&mut a, ProjectItem::SaveAs);
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Move(Dir::Up), 0);
        a.handle(Gesture::Play, 0);
        assert_eq!(a.project.name, "futuroa");
        assert!(!a.read_only);
    }

    #[test]
    fn quit_with_changes_can_save_first() {
        let tmp = TempDir::new();
        let mut a = app_with_data(&tmp);
        a.handle(Gesture::Insert, 0);
        a.handle(Gesture::Quit, 0);
        assert!(a.confirming_quit());
        a.handle(Gesture::Quit, 0);
        assert!(
            a.confirming_quit(),
            "mantener de nuevo no cierra el diálogo"
        );
        a.handle(Gesture::Play, 0);
        assert!(a.quit);
        assert!(data(&a).load("proyecto-001", false).is_ok());
    }

    #[test]
    fn settings_go_to_the_config() {
        let tmp = TempDir::new();
        let mut a = app_with_data(&tmp);
        on_project_item(&mut a, ProjectItem::Font);
        a.handle(Gesture::Insert, 0);
        on_project_item(&mut a, ProjectItem::Follow);
        a.handle(Gesture::Insert, 0);
        assert!(!a.modified, "los ajustes no son del proyecto");
        let b = app_with_data(&tmp);
        assert_eq!(b.font, FontSize::Large);
        assert!(!b.follow);
    }
}
