//! Modo selección, portapapeles, copiar, cortar y pegar (H10; spec-ui, "Modo selección").
//!
//! Cada pantalla define su selección como un rectángulo en sus propias coordenadas `(x, y)`:
//!
//! | Pantalla | x | y | Niveles de Y |
//! |---|---|---|---|
//! | Grilla | paso | pista | celda → pista → sección |
//! | Frase | columna (NOTA…LARGO) | paso | celda → columna → frase |
//! | Canción | pista | fila | celda → fila → bloque |
//! | Cadena | — | posición | posiciones → cadena |
//! | Instrumento | — | — | el instrumento |
//!
//! Qué se copia: en la Grilla y la Frase, **valores** de pasos; en la Canción y la Cadena,
//! **referencias** (números de cadena y de frase, que se comparten como con A); en Instrumento,
//! los parámetros. Cortar en la Canción y la Cadena quita referencias: las cadenas y las frases
//! siguen existiendo. Las ediciones llegan al motor solas (`EngineMirror` compara el proyecto).
//!
//! La lógica sobre el proyecto son funciones libres (`copy_steps`, `paste_steps`...) para
//! probarlas sin `App`; el `impl App` de abajo las conecta con los gestos.

use std::ops::RangeInclusive;

use crate::app::{App, ChainCol, GridCol, PhraseCol, Screen};
use crate::gestures::{Dir, Gesture};
use crate::instrument::Instrument;
use crate::model::{
    CHAIN_LEN, ChainEntry, ChainId, Chord, DEFAULT_GATE, DEFAULT_VELOCITY, EditError,
    MAX_SONG_ROWS, PHRASE_STEPS, Project, Section, Step, TRACKS, TrackKind,
};

/// Rectángulo inclusivo en las coordenadas de la pantalla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x0: usize,
    pub x1: usize,
    pub y0: usize,
    pub y1: usize,
}

impl Area {
    fn spanning(a: (usize, usize), b: (usize, usize)) -> Self {
        Self {
            x0: a.0.min(b.0),
            x1: a.0.max(b.0),
            y0: a.1.min(b.1),
            y1: a.1.max(b.1),
        }
    }

    pub fn contains(&self, x: usize, y: usize) -> bool {
        self.xs().contains(&x) && self.ys().contains(&y)
    }

    pub fn xs(&self) -> RangeInclusive<usize> {
        self.x0..=self.x1
    }

    pub fn ys(&self) -> RangeInclusive<usize> {
        self.y0..=self.y1
    }

    pub fn width(&self) -> usize {
        self.x1 - self.x0 + 1
    }

    pub fn height(&self) -> usize {
        self.y1 - self.y0 + 1
    }
}

/// Lo que agranda Y: la celda (o el rectángulo de la cruz), la línea entera y todo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Cell,
    Line,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub screen: Screen,
    /// Donde estaba el cursor al apretar Y; la cruz extiende desde acá.
    pub anchor: (usize, usize),
    pub level: Level,
}

/// Columnas de la Frase incluidas (índices de `PhraseCol`, inclusivo).
pub type ColSpan = (usize, usize);
pub const ALL_COLS: ColSpan = (0, PhraseCol::ALL.len() - 1);

fn has_col(cols: ColSpan, col: PhraseCol) -> bool {
    (cols.0..=cols.1).contains(&col.index())
}

#[derive(Clone, Debug, PartialEq)]
pub enum Clip {
    /// Pasos: una fila por pista (la Frase copia una sola) con las columnas `cols`.
    Steps {
        cols: ColSpan,
        tracks: Vec<Vec<Step>>,
    },
    /// Entradas de cadena (frase y transposición), contiguas.
    ChainEntries(Vec<ChainEntry>),
    /// Celdas de la canción: filas × pistas.
    SongCells(Vec<Vec<Option<ChainId>>>),
    Instrument(Instrument),
}

impl Clip {
    /// Qué hay en el portapapeles y dónde se pega (para los avisos).
    fn describe(&self) -> &'static str {
        match self {
            Clip::Steps { .. } => "pasos: se pegan en la Grilla o la Frase",
            Clip::ChainEntries(_) => "entradas de cadena: se pegan en la Cadena",
            Clip::SongCells(_) => "celdas de la canción: se pegan en la Canción",
            Clip::Instrument(_) => "un instrumento: se pega en Instrumento",
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

// --- Pasos (Grilla y Frase) ---------------------------------------------------

/// Copia los pasos `steps` de las pistas `tracks` en la sección. Una pista sin frase copia
/// pasos vacíos (pegarlos borra).
pub fn copy_steps(
    project: &Project,
    sec: Section,
    tracks: RangeInclusive<usize>,
    steps: RangeInclusive<usize>,
    cols: ColSpan,
) -> Clip {
    let tracks = tracks
        .map(|t| {
            steps
                .clone()
                .map(|i| project.step(t, sec, i).copied().unwrap_or_default())
                .collect()
        })
        .collect();
    Clip::Steps { cols, tracks }
}

/// Borra las columnas `cols` de los pasos (con NOTA, el paso entero, como A + B).
pub fn cut_steps(
    project: &mut Project,
    sec: Section,
    tracks: RangeInclusive<usize>,
    steps: RangeInclusive<usize>,
    cols: ColSpan,
) {
    for t in tracks {
        for i in steps.clone() {
            let Some(step) = project.step_mut(t, sec, i) else {
                break;
            };
            if has_col(cols, PhraseCol::Note) {
                *step = Step::default();
            } else if step.root().is_some() {
                if has_col(cols, PhraseCol::Chord) {
                    step.chord = Chord::default();
                }
                if has_col(cols, PhraseCol::Velocity) {
                    step.velocity = DEFAULT_VELOCITY;
                }
                if has_col(cols, PhraseCol::Gate) {
                    step.gate_ticks = DEFAULT_GATE;
                }
            }
        }
    }
}

/// Escribe en `dst` las columnas `cols` de `src`. Sin NOTA, solo cambia pasos con nota y
/// solo toma valores de pasos con nota (un paso vacío no tiene velocidad ni largo propios).
fn apply_cols(dst: &mut Step, src: &Step, cols: ColSpan, drum: bool) {
    if cols == ALL_COLS {
        *dst = *src;
    } else {
        if has_col(cols, PhraseCol::Note) {
            if src.is_empty() {
                *dst = Step::default();
                return;
            }
            dst.note = src.note;
        }
        if dst.root().is_none() || src.root().is_none() {
            return;
        }
        if has_col(cols, PhraseCol::Chord) {
            dst.chord = src.chord;
        }
        if has_col(cols, PhraseCol::Velocity) {
            dst.velocity = src.velocity;
        }
        if has_col(cols, PhraseCol::Gate) {
            dst.gate_ticks = src.gate_ticks;
        }
    }
    if drum {
        dst.chord = Chord::default();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct StepsPasted {
    pub steps: usize,
    pub tracks: usize,
    /// Pistas que no se pudieron crear (creación implícita), con el motivo.
    pub skipped: Vec<(usize, EditError)>,
}

/// Pega los pasos con la esquina en `(track0, step0)`; lo que pasa de la pista 8 o del paso
/// 16 se descarta. Crea la frase (y la cadena o la fila) que falte si hay notas para poner.
pub fn paste_steps(
    project: &mut Project,
    sec: Section,
    track0: usize,
    step0: usize,
    cols: ColSpan,
    rows: &[Vec<Step>],
) -> StepsPasted {
    let mut out = StepsPasted::default();
    for (dt, row) in rows.iter().enumerate() {
        let t = track0 + dt;
        if t >= TRACKS {
            break;
        }
        let creates = has_col(cols, PhraseCol::Note) && row.iter().any(|s| !s.is_empty());
        if project.phrase_id_at(t, sec).is_none() {
            if !creates {
                continue;
            }
            if let Err(e) = project.ensure_phrase(t, sec, None) {
                out.skipped.push((t, e));
                continue;
            }
        }
        let drum = project.tracks[t].kind() == TrackKind::Drum;
        let mut wrote = 0;
        for (di, src) in row.iter().enumerate() {
            let Some(dst) = project.step_mut(t, sec, step0 + di) else {
                break;
            };
            apply_cols(dst, src, cols, drum);
            wrote += 1;
        }
        out.steps += wrote;
        out.tracks += usize::from(wrote > 0);
    }
    out
}

// --- Canción ------------------------------------------------------------------

pub fn copy_song_cells(
    project: &Project,
    rows: RangeInclusive<usize>,
    tracks: RangeInclusive<usize>,
) -> Clip {
    let cells = rows
        .map(|r| tracks.clone().map(|t| project.chain_id_at(t, r)).collect())
        .collect();
    Clip::SongCells(cells)
}

/// Vacía las celdas: quita las referencias, las cadenas siguen existiendo.
pub fn cut_song_cells(
    project: &mut Project,
    rows: RangeInclusive<usize>,
    tracks: RangeInclusive<usize>,
) {
    for r in rows {
        let Some(row) = project.song.rows.get_mut(r) else {
            break;
        };
        for t in tracks.clone() {
            row[t] = None;
        }
    }
}

/// Pega las celdas con la esquina en `(row0, track0)`, creando las filas que falten (no más de
/// `MAX_SONG_ROWS`). Las celdas vacías también se pegan (vacían). Devuelve cuántas escribió.
pub fn paste_song_cells(
    project: &mut Project,
    row0: usize,
    track0: usize,
    cells: &[Vec<Option<ChainId>>],
) -> usize {
    let mut wrote = 0;
    for (dr, src) in cells.iter().enumerate() {
        let r = row0 + dr;
        if r >= MAX_SONG_ROWS {
            break;
        }
        // Una fila vacía al final de la canción no se crea; una en el medio, sí.
        let needed = src.iter().any(Option::is_some) || r < project.song.rows.len();
        if !needed {
            wrote += src.len().min(TRACKS.saturating_sub(track0));
            continue;
        }
        while project.song.rows.len() <= r {
            project.song.rows.push([None; TRACKS]);
        }
        for (dt, &cell) in src.iter().enumerate() {
            let t = track0 + dt;
            if t >= TRACKS {
                break;
            }
            if let Some(id) = cell {
                project.chains[usize::from(id)].get_or_insert_with(Default::default);
            }
            project.song.rows[r][t] = cell;
            wrote += 1;
        }
    }
    wrote
}

// --- Cadena -------------------------------------------------------------------

/// Las entradas existentes de las posiciones `positions`.
pub fn copy_chain_entries(
    project: &Project,
    track: usize,
    row: usize,
    positions: RangeInclusive<usize>,
) -> Vec<ChainEntry> {
    let Some(chain) = project.chain_at(track, row) else {
        return Vec::new();
    };
    positions.filter_map(|k| chain.entry(k)).collect()
}

/// Quita las entradas (las siguientes suben). Las frases siguen existiendo.
pub fn cut_chain_entries(
    project: &mut Project,
    track: usize,
    row: usize,
    positions: RangeInclusive<usize>,
) -> usize {
    let at = Section {
        row,
        entry: *positions.start(),
    };
    positions
        .take_while(|_| project.remove_entry(track, at))
        .count()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntriesPasted {
    pub entries: usize,
    pub new_chain: Option<ChainId>,
}

/// Pega las entradas desde `pos`, reemplazando las que había; lo que pasa de la posición 16
/// se descarta. Si la fila no tiene cadena, la crea (creación implícita). Como las entradas
/// son contiguas, no se puede pegar después de un hueco.
pub fn paste_chain_entries(
    project: &mut Project,
    track: usize,
    row: usize,
    pos: usize,
    entries: &[ChainEntry],
) -> Result<EntriesPasted, EditError> {
    let existing = project.chain_id_at(track, row);
    let len = existing
        .and_then(|id| project.chain(id))
        .map_or(0, |c| c.len());
    if pos > len {
        return Err(EditError::Gap { chain_len: len });
    }
    let new_chain = match existing {
        Some(_) => None,
        None => {
            let id = project.free_chain().ok_or(EditError::NoFreeChain)?;
            project.set_song_cell(row, track, Some(id))?;
            Some(id)
        }
    };
    let id = project.chain_id_at(track, row).ok_or(EditError::NoChain)?;
    let chain = project.chains[usize::from(id)].get_or_insert_with(Default::default);
    let mut wrote = 0;
    for (slot, &e) in chain.entries[pos..].iter_mut().zip(entries) {
        *slot = Some(e);
        wrote += 1;
    }
    for e in &entries[..wrote] {
        project.phrases[usize::from(e.phrase)].get_or_insert_with(Default::default);
    }
    Ok(EntriesPasted {
        entries: wrote,
        new_chain,
    })
}

// --- La interfaz ---------------------------------------------------------------

/// Última fila navegable de la Canción: las existentes más una nueva.
fn last_song_row(project: &Project) -> usize {
    project.song.rows.len().min(MAX_SONG_ROWS - 1)
}

fn row_has_cells(project: &Project, row: usize) -> bool {
    project
        .song
        .rows
        .get(row)
        .is_some_and(|r| r.iter().any(Option::is_some))
}

/// Filas del bloque de filas no vacías contiguas que contiene a `y0..=y1` (cualquier pista).
fn song_block(project: &Project, y0: usize, y1: usize) -> (usize, usize) {
    let (mut start, mut end) = (y0, y1);
    if row_has_cells(project, start) {
        while start > 0 && row_has_cells(project, start - 1) {
            start -= 1;
        }
    }
    if row_has_cells(project, end) {
        while row_has_cells(project, end + 1) {
            end += 1;
        }
    }
    (start, end)
}

fn selectable(screen: Screen) -> bool {
    matches!(
        screen,
        Screen::Grid | Screen::Phrase | Screen::Song | Screen::Chain | Screen::Instrument
    )
}

impl App {
    /// Cursor en las coordenadas de selección de la pantalla actual.
    fn selection_cursor(&self) -> (usize, usize) {
        match self.screen {
            Screen::Grid => match self.grid_col {
                GridCol::Step(i) => (i, self.track),
                GridCol::Phrase => (0, self.track),
            },
            Screen::Phrase => (self.phrase_col.index(), self.phrase_row),
            Screen::Song => (self.track, self.section.row),
            Screen::Chain => (0, self.section.entry),
            _ => (0, 0),
        }
    }

    /// Lo seleccionado, en las coordenadas de la pantalla (ver la tabla del módulo).
    pub fn selection_area(&self) -> Option<Area> {
        let sel = self.selection?;
        let mut a = Area::spanning(sel.anchor, self.selection_cursor());
        let full_steps = (0, PHRASE_STEPS - 1);
        match (sel.screen, sel.level) {
            (Screen::Grid, Level::Line) => (a.x0, a.x1) = full_steps,
            (Screen::Grid, Level::All) => {
                (a.x0, a.x1) = full_steps;
                (a.y0, a.y1) = (0, TRACKS - 1);
            }
            (Screen::Phrase, Level::Line) => (a.y0, a.y1) = full_steps,
            (Screen::Phrase, Level::All) => {
                (a.x0, a.x1) = ALL_COLS;
                (a.y0, a.y1) = full_steps;
            }
            (Screen::Song, Level::Line) => (a.x0, a.x1) = (0, TRACKS - 1),
            (Screen::Song, Level::All) => {
                (a.x0, a.x1) = (0, TRACKS - 1);
                (a.y0, a.y1) = song_block(&self.project, a.y0, a.y1);
            }
            (Screen::Chain, level) => {
                (a.x0, a.x1) = (0, 1);
                let len = self
                    .project
                    .chain_at(self.track, self.section.row)
                    .map_or(0, |c| c.len());
                if level == Level::All && len > 0 {
                    (a.y0, a.y1) = (0, len - 1);
                }
            }
            (Screen::Instrument, _) => a = Area::spanning((0, 0), (0, 0)),
            _ => {}
        }
        Some(a)
    }

    /// Tamaño de la selección para la línea de ayuda ("2 pistas × 4 pasos").
    pub fn selection_label(&self) -> Option<String> {
        let sel = self.selection?;
        let a = self.selection_area()?;
        Some(match sel.screen {
            Screen::Grid => format!(
                "{} × {}",
                plural(a.height(), "pista", "pistas"),
                plural(a.width(), "paso", "pasos")
            ),
            Screen::Phrase => format!(
                "{} × {}",
                plural(a.height(), "paso", "pasos"),
                plural(a.width(), "columna", "columnas")
            ),
            Screen::Song => format!(
                "{} × {}",
                plural(a.height(), "fila", "filas"),
                plural(a.width(), "pista", "pistas")
            ),
            Screen::Chain => plural(a.height(), "posición", "posiciones"),
            _ => format!("instrumento de la pista {}", self.track + 1),
        })
    }

    /// A qué amplía el próximo Y (None si ya está todo).
    pub fn selection_next_level(&self) -> Option<&'static str> {
        let sel = self.selection?;
        match (sel.screen, sel.level) {
            (Screen::Grid, Level::Cell) => Some("pista"),
            (Screen::Grid, Level::Line) => Some("sección"),
            (Screen::Phrase, Level::Cell) => Some("columna"),
            (Screen::Phrase, Level::Line) => Some("frase"),
            (Screen::Song, Level::Cell) => Some("fila"),
            (Screen::Song, Level::Line) => Some("bloque"),
            (Screen::Chain, Level::Cell) => Some("cadena"),
            _ => None,
        }
    }

    /// Y fuera del modo selección.
    pub(crate) fn start_selection(&mut self) {
        if !selectable(self.screen) {
            self.info("esta pantalla no tiene selección");
            return;
        }
        let mut level = match self.screen {
            Screen::Instrument => Level::All,
            _ => Level::Cell,
        };
        // Desde la columna FRA se selecciona la pista entera (FRA no es un paso).
        if self.screen == Screen::Grid && self.grid_col == GridCol::Phrase {
            self.grid_col = GridCol::Step(0);
            level = Level::Line;
        }
        self.selection = Some(Selection {
            screen: self.screen,
            anchor: self.selection_cursor(),
            level,
        });
    }

    /// Gestos con la selección abierta. Devuelve `true` si el gesto se usó acá; si no, sigue
    /// por el camino normal (Start, mute, tempo...). Cambiar de pantalla, de sección o de pista
    /// (donde la pista no es un eje de la selección) la cancela.
    pub(crate) fn handle_selection(&mut self, g: Gesture) -> bool {
        let Some(sel) = self.selection else {
            return false;
        };
        match g {
            Gesture::Move(dir) => self.extend(dir),
            Gesture::Track(d) if matches!(sel.screen, Screen::Grid | Screen::Song) => {
                let dir = match (sel.screen, d < 0) {
                    (Screen::Grid, true) => Dir::Up,
                    (Screen::Grid, false) => Dir::Down,
                    (_, true) => Dir::Left,
                    (_, false) => Dir::Right,
                };
                self.extend(dir);
            }
            Gesture::SelectMode => {
                if self.selection_next_level().is_some() {
                    let level = match sel.level {
                        Level::Cell if sel.screen == Screen::Chain => Level::All,
                        Level::Cell => Level::Line,
                        _ => Level::All,
                    };
                    self.selection = Some(Selection { level, ..sel });
                }
            }
            Gesture::Insert => {
                self.copy_selection();
                self.selection = None;
                self.just_copied = true;
            }
            Gesture::Listen => self.cut_selection(),
            Gesture::Cancel => self.selection = None,
            Gesture::Screen(_) | Gesture::Section(_) | Gesture::Track(_) | Gesture::Quit => {
                self.selection = None;
                return false;
            }
            Gesture::Play
            | Gesture::LoopSection
            | Gesture::Mute
            | Gesture::Solo
            | Gesture::Tempo(_)
            | Gesture::Octave(_) => return false,
            // A + cruz (transponer la selección) es v1; el resto no hace nada.
            _ => {}
        }
        true
    }

    /// La cruz mueve el cursor sin salir de la zona que se puede seleccionar.
    fn extend(&mut self, dir: Dir) {
        let up = |v: usize| v.saturating_sub(1);
        let down = |v: usize, max: usize| (v + 1).min(max);
        match self.screen {
            Screen::Grid => {
                let i = match self.grid_col {
                    GridCol::Step(i) => i,
                    GridCol::Phrase => 0,
                };
                match dir {
                    Dir::Up => self.track = up(self.track),
                    Dir::Down => self.track = down(self.track, TRACKS - 1),
                    Dir::Left => self.grid_col = GridCol::Step(up(i)),
                    Dir::Right => self.grid_col = GridCol::Step(down(i, PHRASE_STEPS - 1)),
                }
            }
            Screen::Phrase => {
                let c = self.phrase_col.index();
                match dir {
                    Dir::Up => self.phrase_row = up(self.phrase_row),
                    Dir::Down => self.phrase_row = down(self.phrase_row, PHRASE_STEPS - 1),
                    Dir::Left => self.phrase_col = PhraseCol::ALL[up(c)],
                    Dir::Right => self.phrase_col = PhraseCol::ALL[down(c, ALL_COLS.1)],
                }
            }
            Screen::Song => match dir {
                Dir::Up | Dir::Down => {
                    let row = self.section.row;
                    let last = last_song_row(&self.project);
                    let row = if dir == Dir::Up {
                        up(row)
                    } else {
                        down(row, last)
                    };
                    self.section = Section { row, entry: 0 };
                }
                Dir::Left => self.track = up(self.track),
                Dir::Right => self.track = down(self.track, TRACKS - 1),
            },
            Screen::Chain => match dir {
                Dir::Up => self.section.entry = up(self.section.entry),
                Dir::Down => self.section.entry = down(self.section.entry, CHAIN_LEN - 1),
                Dir::Left => self.chain_col = ChainCol::Phrase,
                Dir::Right => self.chain_col = ChainCol::Transpose,
            },
            _ => {}
        }
    }

    /// Copia lo seleccionado al portapapeles. Devuelve cuántos elementos copió.
    fn copy_selection(&mut self) -> Option<usize> {
        let sel = self.selection?;
        let a = self.selection_area()?;
        let (clip, n, text) = match sel.screen {
            Screen::Grid => {
                let clip = copy_steps(&self.project, self.section, a.ys(), a.xs(), ALL_COLS);
                let n = a.width() * a.height();
                (
                    clip,
                    n,
                    format!(
                        "{} ({})",
                        plural(n, "paso", "pasos"),
                        plural(a.height(), "pista", "pistas")
                    ),
                )
            }
            Screen::Phrase => {
                let cols = (a.x0, a.x1);
                let clip = copy_steps(
                    &self.project,
                    self.section,
                    self.track..=self.track,
                    a.ys(),
                    cols,
                );
                let n = a.height();
                let what = if cols == ALL_COLS {
                    String::new()
                } else {
                    let names: Vec<&str> = PhraseCol::ALL[a.x0..=a.x1]
                        .iter()
                        .map(|&c| col_name(c))
                        .collect();
                    format!(" (solo {})", names.join(", "))
                };
                (clip, n, format!("{}{what}", plural(n, "paso", "pasos")))
            }
            Screen::Song => {
                let clip = copy_song_cells(&self.project, a.ys(), a.xs());
                let n = a.width() * a.height();
                (
                    clip,
                    n,
                    plural(n, "celda de la canción", "celdas de la canción"),
                )
            }
            Screen::Chain => {
                let entries =
                    copy_chain_entries(&self.project, self.track, self.section.row, a.ys());
                if entries.is_empty() {
                    self.warn("no hay entradas de cadena para copiar");
                    return None;
                }
                let n = entries.len();
                (
                    Clip::ChainEntries(entries),
                    n,
                    plural(n, "entrada de cadena", "entradas de cadena"),
                )
            }
            Screen::Instrument => {
                let track = &self.project.tracks[self.track];
                let text = format!("instrumento de la pista {} {}", self.track + 1, track.name);
                (Clip::Instrument(track.instrument), 1, text)
            }
            _ => return None,
        };
        self.clipboard = Some(clip);
        self.info(format!("copiado: {text}"));
        Some(n)
    }

    /// X: copia y borra (en la Canción y la Cadena, quita las referencias).
    fn cut_selection(&mut self) {
        let Some(sel) = self.selection else { return };
        if sel.screen == Screen::Instrument {
            self.warn("un instrumento no se corta: A lo copia, B cancela");
            return;
        }
        let Some(a) = self.selection_area() else {
            return;
        };
        let Some(n) = self.copy_selection() else {
            self.selection = None;
            return;
        };
        self.selection = None;
        let (sec, track) = (self.section, self.track);
        let text = match sel.screen {
            Screen::Grid => {
                cut_steps(&mut self.project, sec, a.ys(), a.xs(), ALL_COLS);
                format!("cortado: {}", plural(n, "paso", "pasos"))
            }
            Screen::Phrase => {
                cut_steps(&mut self.project, sec, track..=track, a.ys(), (a.x0, a.x1));
                format!("cortado: {}", plural(n, "paso", "pasos"))
            }
            Screen::Song => {
                cut_song_cells(&mut self.project, a.ys(), a.xs());
                format!(
                    "cortado: {} vaciadas (las cadenas siguen existiendo)",
                    plural(n, "celda", "celdas")
                )
            }
            Screen::Chain => {
                let removed = cut_chain_entries(&mut self.project, track, sec.row, a.ys());
                self.section.entry = a.y0.min(self.section.entry);
                format!(
                    "cortado: {} quitadas de la cadena (las frases siguen existiendo)",
                    plural(removed, "entrada", "entradas")
                )
            }
            _ => return,
        };
        self.modified = true;
        self.info(text);
    }

    /// Select + A: pega en la posición del cursor si el portapapeles es del tipo de la pantalla.
    pub(crate) fn paste(&mut self) {
        let Some(clip) = self.clipboard.clone() else {
            self.warn("el portapapeles está vacío: Y selecciona, A copia");
            return;
        };
        let result = match (self.screen, &clip) {
            (Screen::Grid, Clip::Steps { cols, tracks }) => {
                let step0 = match self.grid_col {
                    GridCol::Step(i) => i,
                    GridCol::Phrase => 0,
                };
                Ok(self.paste_steps_report(self.track, step0, *cols, tracks))
            }
            (Screen::Phrase, Clip::Steps { cols, tracks }) => match tracks.as_slice() {
                [one] => Ok(self.paste_steps_report(
                    self.track,
                    self.phrase_row,
                    *cols,
                    std::slice::from_ref(one),
                )),
                many => Err(format!(
                    "el portapapeles tiene {} pistas: pegalo en la Grilla",
                    many.len()
                )),
            },
            (Screen::Song, Clip::SongCells(cells)) => {
                let n = paste_song_cells(&mut self.project, self.section.row, self.track, cells);
                Ok(Pasted::info(format!(
                    "pegado: {}",
                    plural(n, "celda", "celdas")
                )))
            }
            (Screen::Chain, Clip::ChainEntries(entries)) => {
                let (track, row, pos) = (self.track, self.section.row, self.section.entry);
                match paste_chain_entries(&mut self.project, track, row, pos, entries) {
                    Ok(p) => {
                        let created = p
                            .new_chain
                            .map_or(String::new(), |c| format!(" en la cadena nueva {c:03}"));
                        Ok(Pasted::info(format!(
                            "pegado: {}{created}",
                            plural(p.entries, "entrada", "entradas")
                        )))
                    }
                    Err(EditError::Gap { chain_len }) => Err(format!(
                        "la cadena tiene {}: pegá en la posición {} o antes",
                        plural(chain_len, "frase", "frases"),
                        chain_len + 1
                    )),
                    Err(e) => Err(e.to_string()),
                }
            }
            (Screen::Instrument, Clip::Instrument(inst)) => {
                let track = &mut self.project.tracks[self.track];
                track.instrument = *inst;
                Ok(Pasted::info(format!(
                    "pegado: instrumento en la pista {} {}",
                    self.track + 1,
                    track.name
                )))
            }
            (screen, clip) if selectable(screen) => {
                Err(format!("el portapapeles tiene {}", clip.describe()))
            }
            _ => Err("en esta pantalla no se pega nada".to_string()),
        };
        match result {
            Ok(Pasted { text, warn }) => {
                self.modified = true;
                if warn {
                    self.warn(text);
                } else {
                    self.info(text);
                }
            }
            Err(text) => self.warn(text),
        }
    }

    /// Pega pasos y arma el mensaje (un aviso si alguna pista no se pudo crear).
    fn paste_steps_report(
        &mut self,
        track0: usize,
        step0: usize,
        cols: ColSpan,
        rows: &[Vec<Step>],
    ) -> Pasted {
        let out = paste_steps(&mut self.project, self.section, track0, step0, cols, rows);
        let text = format!(
            "pegado: {} en {}",
            plural(out.steps, "paso", "pasos"),
            plural(out.tracks, "pista", "pistas")
        );
        match out.skipped.first() {
            Some((t, e)) => Pasted {
                text: format!("{text}; pista {}: {e}", t + 1),
                warn: true,
            },
            None => Pasted::info(text),
        }
    }
}

/// Resultado de un pegado para la línea de mensajes.
struct Pasted {
    text: String,
    warn: bool,
}

impl Pasted {
    fn info(text: String) -> Self {
        Self { text, warn: false }
    }
}

fn col_name(c: PhraseCol) -> &'static str {
    match c {
        PhraseCol::Note => "NOTA",
        PhraseCol::Chord => "ACORDE",
        PhraseCol::Velocity => "VEL",
        PhraseCol::Gate => "LARGO",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChordKind, NoteCell};

    const S0: Section = Section { row: 0, entry: 0 };

    fn note(n: u8) -> Step {
        Step {
            note: NoteCell::On(n),
            ..Step::default()
        }
    }

    fn app() -> App {
        App::new(Project::new_default())
    }

    fn g(app: &mut App, gesture: Gesture) {
        app.handle(gesture, 0);
    }

    fn mv(app: &mut App, dir: Dir, times: usize) {
        for _ in 0..times {
            g(app, Gesture::Move(dir));
        }
    }

    fn put(app: &mut App, track: usize, sec: Section, i: usize, n: u8) {
        app.project.ensure_phrase(track, sec, None).unwrap();
        *app.project.step_mut(track, sec, i).unwrap() = note(n);
    }

    fn notes(p: &Project, track: usize, sec: Section) -> Vec<Option<u8>> {
        (0..PHRASE_STEPS)
            .map(|i| p.step(track, sec, i).and_then(Step::root))
            .collect()
    }

    // --- Lógica pura ---

    #[test]
    fn copy_and_paste_steps_between_tracks() {
        let mut p = Project::new_default();
        *p.step_mut(0, S0, 0).unwrap() = note(60);
        *p.step_mut(0, S0, 2).unwrap() = note(62);
        let Clip::Steps { cols, tracks } = copy_steps(&p, S0, 0..=0, 0..=3, ALL_COLS) else {
            panic!()
        };
        let out = paste_steps(&mut p, S0, 1, 4, cols, &tracks);
        assert_eq!(out.steps, 4);
        assert_eq!(out.tracks, 1);
        let n = notes(&p, 1, S0);
        assert_eq!((n[4], n[5], n[6]), (Some(60), None, Some(62)));
        assert_eq!(
            notes(&p, 0, S0)[0],
            Some(60),
            "copiar no cambia el original"
        );
    }

    #[test]
    fn paste_steps_is_clipped_at_the_edges() {
        let mut p = Project::new_default();
        let rows = vec![vec![note(60); 4]; 3];
        // Desde la pista 7, paso 14: entra 1 pista × 2 pasos.
        let out = paste_steps(&mut p, S0, 7, 14, ALL_COLS, &rows);
        assert_eq!(out.steps, 2);
        assert_eq!(out.tracks, 1);
        assert_eq!(notes(&p, 7, S0)[13..], [None, Some(60), Some(60)]);
    }

    #[test]
    fn paste_steps_creates_missing_phrases_and_reports_gaps() {
        let mut p = Project::new_default();
        let rows = vec![vec![note(60)], vec![Step::default()]];
        // Sección 1 (nueva): la pista 0 recibe frase; la 1 solo tiene pasos vacíos y no se crea.
        let s1 = Section { row: 0, entry: 1 };
        let out = paste_steps(&mut p, s1, 0, 0, ALL_COLS, &rows);
        assert_eq!(out.tracks, 1);
        assert!(p.phrase_id_at(0, s1).is_some());
        assert!(p.phrase_id_at(1, s1).is_none());
        // Sección 3: hueco en la cadena; no se crea nada y se informa.
        let s3 = Section { row: 0, entry: 3 };
        let out = paste_steps(&mut p, s3, 2, 0, ALL_COLS, &rows[..1]);
        assert_eq!(out.steps, 0);
        assert_eq!(out.skipped, vec![(2, EditError::Gap { chain_len: 1 })]);
    }

    #[test]
    fn partial_columns_touch_only_their_fields_and_notes() {
        let mut p = Project::new_default();
        let loud = Step {
            velocity: 30,
            gate_ticks: 96,
            ..note(64)
        };
        *p.step_mut(7, S0, 0).unwrap() = loud;
        *p.step_mut(7, S0, 1).unwrap() = note(60);
        // VEL y LARGO de los pasos 0–2 (el 2 está vacío: no aporta valores).
        let Clip::Steps { cols, tracks } = copy_steps(&p, S0, 7..=7, 0..=2, (2, 3)) else {
            panic!()
        };
        assert_eq!(cols, (2, 3));
        paste_steps(&mut p, S0, 7, 1, cols, &tracks);
        let s1 = *p.step(7, S0, 1).unwrap();
        assert_eq!((s1.root(), s1.velocity, s1.gate_ticks), (Some(60), 30, 96));
        // El paso 2 (destino del paso 1 de origen): vacío, sigue vacío.
        assert!(p.step(7, S0, 2).unwrap().is_empty());
        // El paso 3 recibe el paso 2 de origen, que estaba vacío: nada cambia.
        assert!(p.step(7, S0, 3).unwrap().is_empty());
    }

    #[test]
    fn pasting_chords_on_drums_drops_them() {
        let mut p = Project::new_default();
        let chord = Step {
            chord: Chord {
                kind: ChordKind::Major,
                inversion: 1,
            },
            ..note(60)
        };
        let out = paste_steps(&mut p, S0, 0, 0, ALL_COLS, &[vec![chord]]);
        assert_eq!(out.steps, 1);
        assert_eq!(p.step(0, S0, 0).unwrap().chord, Chord::default());
        paste_steps(&mut p, S0, 7, 0, ALL_COLS, &[vec![chord]]);
        assert_eq!(p.step(7, S0, 0).unwrap().chord.kind, ChordKind::Major);
    }

    #[test]
    fn cut_steps_by_columns() {
        let mut p = Project::new_default();
        let s = Step {
            velocity: 20,
            gate_ticks: 48,
            ..note(60)
        };
        *p.step_mut(7, S0, 0).unwrap() = s;
        *p.step_mut(7, S0, 1).unwrap() = s;
        cut_steps(&mut p, S0, 7..=7, 0..=0, (2, 2));
        let s0 = *p.step(7, S0, 0).unwrap();
        assert_eq!((s0.root(), s0.velocity, s0.gate_ticks), (Some(60), 100, 48));
        cut_steps(&mut p, S0, 7..=7, 1..=1, (0, 0));
        assert!(
            p.step(7, S0, 1).unwrap().is_empty(),
            "con NOTA, el paso entero"
        );
    }

    #[test]
    fn song_cells_copy_cut_and_paste_references() {
        let mut p = Project::new_default();
        let clip = copy_song_cells(&p, 0..=0, 0..=2);
        let Clip::SongCells(cells) = clip else {
            panic!()
        };
        assert_eq!(cells, vec![vec![Some(0), Some(1), Some(2)]]);
        // Pegar en la fila nueva (1) desde la pista 6: entran 2 celdas y se crea la fila.
        assert_eq!(paste_song_cells(&mut p, 1, 6, &cells), 2);
        assert_eq!(p.song.rows.len(), 2);
        assert_eq!(p.song.rows[1][6..], [Some(0), Some(1)]);
        assert_eq!(p.chain_uses(0), 2, "se comparte la cadena");
        cut_song_cells(&mut p, 0..=0, 0..=1);
        assert_eq!(p.chain_id_at(0, 0), None);
        assert!(p.chain(0).is_some(), "cortar no borra la cadena");
    }

    #[test]
    fn song_paste_keeps_empty_rows_in_the_middle_only() {
        let mut p = Project::new_default();
        let cells = vec![vec![Some(3)], vec![None], vec![Some(4)], vec![None]];
        assert_eq!(paste_song_cells(&mut p, 1, 0, &cells), 4);
        assert_eq!(p.song.rows.len(), 4, "la fila vacía del final no se crea");
        assert_eq!(p.chain_id_at(0, 2), None);
        assert_eq!(p.chain_id_at(0, 3), Some(4));
        // Al final de la canción posible, lo que no entra se descarta.
        let tall = vec![vec![Some(1)]; 3];
        p.song.rows.resize(MAX_SONG_ROWS - 1, [None; TRACKS]);
        assert_eq!(paste_song_cells(&mut p, MAX_SONG_ROWS - 1, 0, &tall), 1);
        assert_eq!(p.song.rows.len(), MAX_SONG_ROWS);
    }

    #[test]
    fn chain_entries_copy_cut_and_paste() {
        let mut p = Project::new_default();
        let e = |phrase, transpose| ChainEntry { phrase, transpose };
        p.ensure_phrase(0, Section { row: 0, entry: 1 }, Some(20))
            .unwrap();
        p.set_entry_transpose(0, Section { row: 0, entry: 1 }, 5)
            .unwrap();
        let copied = copy_chain_entries(&p, 0, 0, 0..=5);
        assert_eq!(copied, vec![e(0, 0), e(20, 5)], "solo las existentes");

        // Pegar al final de la cadena de la pista 1 la extiende.
        let out = paste_chain_entries(&mut p, 1, 0, 1, &copied).unwrap();
        assert_eq!(
            out,
            EntriesPasted {
                entries: 2,
                new_chain: None
            }
        );
        assert_eq!(p.chain_at(1, 0).unwrap().len(), 3);
        // Después de un hueco, no.
        assert_eq!(
            paste_chain_entries(&mut p, 2, 0, 3, &copied),
            Err(EditError::Gap { chain_len: 1 })
        );
        // En una fila sin cadena crea una.
        let out = paste_chain_entries(&mut p, 4, 1, 0, &copied).unwrap();
        assert_eq!(out.new_chain, Some(8));
        assert_eq!(p.chain_at(4, 1).unwrap().entry(1), Some(e(20, 5)));
        // Cerca del final de la cadena se descarta lo que no entra.
        let many = vec![e(1, 0); CHAIN_LEN];
        let out = paste_chain_entries(&mut p, 5, 0, 1, &many).unwrap();
        assert_eq!(out.entries, CHAIN_LEN - 1);

        // Cortar quita las entradas y las siguientes suben; las frases siguen existiendo.
        assert_eq!(cut_chain_entries(&mut p, 1, 0, 0..=1), 2);
        assert_eq!(p.chain_at(1, 0).unwrap().len(), 1);
        assert_eq!(p.chain_at(1, 0).unwrap().entry(0), Some(e(20, 5)));
        assert!(p.phrase(1).is_some());
        // Cortar más allá del final solo cuenta lo que había.
        assert_eq!(cut_chain_entries(&mut p, 1, 0, 0..=9), 1);
    }

    // --- Con la interfaz ---

    #[test]
    fn grid_selection_grows_cell_track_section() {
        let mut a = app();
        mv(&mut a, Dir::Right, 2);
        g(&mut a, Gesture::SelectMode);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 2,
                x1: 2,
                y0: 0,
                y1: 0
            })
        );
        mv(&mut a, Dir::Right, 3);
        mv(&mut a, Dir::Down, 1);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 2,
                x1: 5,
                y0: 0,
                y1: 1
            })
        );
        assert_eq!(a.selection_label().unwrap(), "2 pistas × 4 pasos");
        // Hacia el otro lado del ancla también.
        mv(&mut a, Dir::Left, 5);
        assert_eq!(a.selection_area().unwrap().x0, 0, "no entra en FRA");
        g(&mut a, Gesture::SelectMode);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 0,
                x1: 15,
                y0: 0,
                y1: 1
            })
        );
        assert_eq!(a.selection_next_level(), Some("sección"));
        g(&mut a, Gesture::SelectMode);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 0,
                x1: 15,
                y0: 0,
                y1: 7
            })
        );
        assert_eq!(a.selection_next_level(), None);
        g(&mut a, Gesture::Cancel);
        assert!(a.selection.is_none());
        assert!(a.clipboard.is_none(), "B no copia");
    }

    #[test]
    fn grid_copy_paste_and_cut() {
        let mut a = app();
        put(&mut a, 0, S0, 0, 60);
        put(&mut a, 0, S0, 3, 63);
        a.modified = false;
        // Copiar los pasos 1–4 del bombo y pegarlos en el paso 9.
        g(&mut a, Gesture::SelectMode);
        mv(&mut a, Dir::Right, 3);
        g(&mut a, Gesture::Insert);
        assert!(a.selection.is_none());
        assert!(!a.modified, "copiar no modifica");
        assert_eq!(
            a.message.as_ref().unwrap().text,
            "copiado: 4 pasos (1 pista)"
        );
        // Doble toque de A justo después de copiar: no pone una nota.
        g(&mut a, Gesture::InsertNew);
        assert_eq!(notes(&a.project, 0, S0)[3], Some(63));
        mv(&mut a, Dir::Right, 5);
        g(&mut a, Gesture::Paste);
        assert!(a.modified);
        let n = notes(&a.project, 0, S0);
        assert_eq!((n[8], n[11]), (Some(60), Some(63)));
        assert_eq!(
            a.message.as_ref().unwrap().text,
            "pegado: 4 pasos en 1 pista"
        );
        // Cortar el paso 9: queda vacío y en el portapapeles.
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::Listen);
        assert!(a.selection.is_none());
        assert_eq!(notes(&a.project, 0, S0)[8], None);
        assert_eq!(
            a.clipboard,
            Some(Clip::Steps {
                cols: ALL_COLS,
                tracks: vec![vec![note(60)]]
            })
        );
    }

    #[test]
    fn grid_selection_from_fra_takes_the_whole_track() {
        let mut a = app();
        a.grid_col = GridCol::Phrase;
        g(&mut a, Gesture::SelectMode);
        assert_eq!(a.grid_col, GridCol::Step(0));
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 0,
                x1: 15,
                y0: 0,
                y1: 0
            })
        );
        // L1/R1 extienden por pistas en la Grilla.
        g(&mut a, Gesture::Track(1));
        assert_eq!(a.selection_area().unwrap().y1, 1);
        assert!(a.selection.is_some());
    }

    #[test]
    fn selection_cancels_on_screen_or_section_change_and_lets_globals_through() {
        let mut a = app();
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::Play);
        assert!(a.transport.is_some(), "Start sigue andando");
        assert!(a.selection.is_some());
        g(&mut a, Gesture::Section(1));
        assert!(a.selection.is_none());
        assert_eq!(
            a.section,
            Section { row: 0, entry: 1 },
            "y el gesto se aplica"
        );
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::Screen(Dir::Right));
        assert!(a.selection.is_none());
        assert_eq!(a.screen, Screen::Chain);
    }

    #[test]
    fn phrase_columns_copy_and_paste_to_grid_and_phrase() {
        let mut a = app();
        a.track = 7;
        put(&mut a, 7, S0, 0, 60);
        a.project.step_mut(7, S0, 0).unwrap().velocity = 40;
        put(&mut a, 7, S0, 4, 67);
        a.screen = Screen::Phrase;
        // Columna VEL entera (celda → columna).
        a.phrase_col = PhraseCol::Velocity;
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::SelectMode);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 2,
                x1: 2,
                y0: 0,
                y1: 15
            })
        );
        assert_eq!(a.selection_label().unwrap(), "16 pasos × 1 columna");
        g(&mut a, Gesture::Insert);
        assert_eq!(
            a.message.as_ref().unwrap().text,
            "copiado: 16 pasos (solo VEL)"
        );
        // Pegada 4 pasos más abajo: la nota del paso 4 recibe la velocidad del paso 0.
        a.phrase_row = 4;
        g(&mut a, Gesture::Paste);
        assert_eq!(a.project.step(7, S0, 4).unwrap().velocity, 40);
        assert_eq!(a.project.step(7, S0, 4).unwrap().root(), Some(67));
        // Frase entera (tres Y) copiada y pegada en la Grilla, en la pista 7 (BAJO).
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::SelectMode);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 0,
                x1: 3,
                y0: 0,
                y1: 15
            })
        );
        g(&mut a, Gesture::Insert);
        a.screen = Screen::Grid;
        a.track = 6;
        a.grid_col = GridCol::Step(0);
        g(&mut a, Gesture::Paste);
        assert_eq!(notes(&a.project, 6, S0), notes(&a.project, 7, S0));
    }

    #[test]
    fn multi_track_steps_do_not_paste_in_the_phrase() {
        let mut a = app();
        g(&mut a, Gesture::SelectMode);
        mv(&mut a, Dir::Down, 2);
        g(&mut a, Gesture::Insert);
        a.screen = Screen::Phrase;
        let before = a.project.clone();
        g(&mut a, Gesture::Paste);
        assert_eq!(a.project, before);
        let m = a.message.as_ref().unwrap();
        assert!(m.warn);
        assert_eq!(
            m.text,
            "el portapapeles tiene 3 pistas: pegalo en la Grilla"
        );
    }

    #[test]
    fn incompatible_or_empty_clipboard_is_refused() {
        let mut a = app();
        g(&mut a, Gesture::Paste);
        assert!(a.message.as_ref().unwrap().warn);
        // Celdas de la canción en la Grilla: aviso y nada cambia.
        a.screen = Screen::Song;
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::Insert);
        assert!(matches!(a.clipboard, Some(Clip::SongCells(_))));
        a.screen = Screen::Grid;
        let before = a.project.clone();
        g(&mut a, Gesture::Paste);
        assert_eq!(a.project, before);
        assert!(!a.modified);
        assert_eq!(
            a.message.as_ref().unwrap().text,
            "el portapapeles tiene celdas de la canción: se pegan en la Canción"
        );
        // Proyecto no tiene selección ni pegado.
        a.screen = Screen::Project;
        g(&mut a, Gesture::SelectMode);
        assert!(a.selection.is_none());
    }

    #[test]
    fn song_selection_row_and_block() {
        let mut a = app();
        a.project.song.rows.push([Some(1); TRACKS]);
        a.project.song.rows.push([None; TRACKS]);
        a.project.song.rows.push([Some(2); TRACKS]);
        a.screen = Screen::Song;
        a.track = 3;
        a.section.row = 1;
        g(&mut a, Gesture::SelectMode);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 3,
                x1: 3,
                y0: 1,
                y1: 1
            })
        );
        g(&mut a, Gesture::SelectMode);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 0,
                x1: 7,
                y0: 1,
                y1: 1
            })
        );
        g(&mut a, Gesture::SelectMode);
        assert_eq!(
            a.selection_area(),
            Some(Area {
                x0: 0,
                x1: 7,
                y0: 0,
                y1: 1
            }),
            "el bloque termina en la fila vacía"
        );
        g(&mut a, Gesture::Insert);
        // Pegar el bloque en la fila 4 (la nueva) extiende la canción.
        a.section.row = 4;
        a.track = 0;
        g(&mut a, Gesture::Paste);
        assert_eq!(a.project.song.rows.len(), 6);
        assert_eq!(a.project.song.rows[4], a.project.song.rows[0]);
        assert_eq!(a.project.song.rows[5], [Some(1); TRACKS]);
        // Cortar una celda: la cadena sigue existiendo.
        a.section.row = 3;
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::Listen);
        assert_eq!(a.project.chain_id_at(0, 3), None);
        assert!(a.project.chain(2).is_some());
        assert!(
            a.message
                .as_ref()
                .unwrap()
                .text
                .contains("las cadenas siguen existiendo")
        );
    }

    #[test]
    fn chain_selection_cut_and_paste() {
        let mut a = app();
        a.screen = Screen::Chain;
        for k in 1..4 {
            a.project
                .ensure_phrase(0, Section { row: 0, entry: k }, Some(10 + k as u8))
                .unwrap();
        }
        a.section.entry = 1;
        g(&mut a, Gesture::SelectMode);
        mv(&mut a, Dir::Down, 1);
        assert_eq!(a.selection_label().unwrap(), "2 posiciones");
        g(&mut a, Gesture::Listen);
        let chain = a.project.chain_at(0, 0).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.entry(1).unwrap().phrase, 13, "las siguientes suben");
        assert!(a.project.phrase(11).is_some(), "la frase sigue existiendo");
        assert_eq!(a.section.entry, 1);
        // Pegar al final (posición 3 de la cadena de largo 2 = hueco): aviso.
        a.section.entry = 3;
        g(&mut a, Gesture::Paste);
        assert!(a.message.as_ref().unwrap().warn);
        // En la posición 2 (la primera libre): se agregan.
        a.section.entry = 2;
        g(&mut a, Gesture::Paste);
        let ids: Vec<u8> = (0..4)
            .map(|k| a.project.chain_at(0, 0).unwrap().entry(k).unwrap().phrase)
            .collect();
        assert_eq!(ids, [0, 13, 11, 12]);
        // Toda la cadena con Y Y.
        a.section.entry = 0;
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::SelectMode);
        assert_eq!(a.selection_label().unwrap(), "4 posiciones");
        assert_eq!(a.selection_next_level(), None);
    }

    #[test]
    fn empty_chain_selection_copies_nothing() {
        let mut a = app();
        a.screen = Screen::Chain;
        a.section.entry = 5;
        a.clipboard = Some(Clip::Instrument(a.project.tracks[0].instrument));
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::Insert);
        assert!(a.selection.is_none());
        assert!(
            matches!(a.clipboard, Some(Clip::Instrument(_))),
            "no se pisa"
        );
        assert!(a.message.as_ref().unwrap().warn);
    }

    #[test]
    fn instrument_copy_and_paste() {
        let mut a = app();
        a.screen = Screen::Instrument;
        a.track = 7;
        a.project.tracks[7].instrument.voices = 6;
        g(&mut a, Gesture::SelectMode);
        assert_eq!(a.selection_label().unwrap(), "instrumento de la pista 8");
        g(&mut a, Gesture::Listen);
        assert!(a.selection.is_some(), "X no corta un instrumento");
        g(&mut a, Gesture::Insert);
        a.track = 6;
        g(&mut a, Gesture::Paste);
        assert_eq!(
            a.project.tracks[6].instrument,
            a.project.tracks[7].instrument
        );
        assert_eq!(a.project.tracks[6].kind(), TrackKind::Synth);
        assert!(a.modified);
    }

    #[test]
    fn follow_pauses_while_selecting() {
        use crate::engine::{HeadPos, Playhead};
        let mut a = app();
        let at = |entry| {
            let mut p = Playhead {
                playing: true,
                ..Playhead::default()
            };
            p.heads[0] = HeadPos {
                active: true,
                row: 0,
                entry,
                step: 0,
            };
            p
        };
        a.set_playhead(at(0));
        g(&mut a, Gesture::SelectMode);
        a.set_playhead(at(1));
        assert_eq!(a.section, S0, "la vista no salta con una selección abierta");
        g(&mut a, Gesture::Cancel);
        a.set_playhead(at(2));
        assert_eq!(a.section.entry, 2);
    }

    #[test]
    fn paste_is_seen_by_the_engine_mirror() {
        use crate::engine::Command;
        use crate::sync::EngineMirror;
        let mut a = app();
        let mut mirror = EngineMirror::new();
        mirror.sync(&a.project, None, &mut |_| true);
        put(&mut a, 0, S0, 0, 60);
        g(&mut a, Gesture::SelectMode);
        g(&mut a, Gesture::Insert);
        a.track = 1;
        g(&mut a, Gesture::Paste);
        let mut sent = Vec::new();
        mirror.sync(&a.project, None, &mut |c| {
            sent.push(c);
            true
        });
        assert!(sent.iter().any(|c| matches!(
            c,
            Command::SetStep {
                phrase: 1,
                step: 0,
                ..
            }
        )));
    }
}
