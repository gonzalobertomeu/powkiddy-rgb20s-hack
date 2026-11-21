//! Canción y Cadena (H5), Proyecto (ajustes y prueba de controles), Instrumento (por ahora solo
//! las voces, H4) y la pantalla pendiente (Mezcla).

use super::selection::{SELECT, footer_or_selection, selected};
use super::{
    HEIGHT, Painter, R, View, WIDTH, cursor_color, fit, footer, footer_height, load_text,
    message_line, scroll_start, status_parts, theme, title_bar, title_height,
};
use crate::app::{App, ChainCol, FontSize, ProjectItem};
use crate::input::Button;
use crate::instrument::{DrumModel, Patch};
use crate::model::{
    CHAIN_LEN, ChordKind, MAX_SONG_ROWS, NoteCell, PHRASE_STEPS, TRACKS, TrackKind, note_name,
};

const NAV_HELP: [&str; 4] = [
    "Sel+cruz pantalla",
    "L1/R1 pista",
    "L2/R2 sección",
    "X escuchar",
];

fn title(p: &mut Painter, app: &App, context: &str) -> R {
    let parts = status_parts(app);
    title_bar(p, app.screen, context, &parts)
}

const SONG_HELP: [&str; 9] = [
    "A poner",
    "2xA nueva",
    "A+Y clonar",
    "A+B borrar",
    "X escuchar",
    "Y selec.",
    "Sel+A pegar",
    "L2/R2 página",
    "Start canción",
];

const CHAIN_HELP: [&str; 9] = [
    "A poner",
    "2xA nueva",
    "A+Y clonar",
    "A+B quitar",
    "X escuchar",
    "Y selec.",
    "Sel+A pegar",
    "L2/R2 fila",
    "Start cadena",
];

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Duración de `phrases` frases al tempo del proyecto, como `m:ss`.
fn duration(app: &App, phrases: usize) -> String {
    let secs = phrases as f32 * PHRASE_STEPS as f32 * 15.0 / app.project.tempo_bpm;
    let secs = secs.round() as u32;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Texto del aviso de desalineación de una fila (spec-ui, Canción).
fn alignment_text(app: &App, row: usize) -> String {
    let project = &app.project;
    let lens = project.chain_lengths(row);
    let present: Vec<(usize, usize)> = lens
        .iter()
        .enumerate()
        .filter_map(|(t, l)| l.map(|l| (t, l)))
        .collect();
    let Some(&(_, first)) = present.first() else {
        return format!("fila {row:02}: vacía");
    };
    if !project.row_misaligned(row) {
        return format!(
            "fila {row:02}: todas las cadenas duran {}",
            plural(first, "frase", "frases")
        );
    }
    // El largo más común es "el resto"; se nombran las pistas que difieren.
    let mut counts: Vec<(usize, usize)> = Vec::new();
    for &(_, l) in &present {
        match counts.iter_mut().find(|(len, _)| *len == l) {
            Some((_, n)) => *n += 1,
            None => counts.push((l, 1)),
        }
    }
    let common = counts.iter().max_by_key(|(_, n)| *n).map_or(first, |c| c.0);
    let odd: Vec<String> = present
        .iter()
        .filter(|(_, l)| *l != common)
        .map(|&(t, l)| format!("{} {l}", fit(&project.tracks[t].name, 5)))
        .collect();
    format!(
        "! fila {row:02}: {}; las demás {common}. Las pistas se desalinean",
        odd.join(", ")
    )
}

/// Canción: filas × pistas con el número de cadena, el largo de cada cadena y los cabezales.
pub fn song(p: &mut Painter, app: &App, view: &View) -> R {
    let project = &app.project;
    let row = app.section.row;
    title(
        p,
        app,
        &format!("fila {row:02}/{}", project.song.rows.len()),
    )?;
    let (cw, ch) = (p.cw, p.ch);
    // Las columnas se ubican en píxeles para que entren con las dos fuentes; el largo de las
    // cadenas va siempre con la fuente chica.
    let row_label_w = 4 * cw;
    let first_x = 8 + row_label_w + cw;
    let lens_x = WIDTH - 8 - 8 * 18 - 16;
    let track_w = (lens_x - 8 - first_x) / TRACKS as i32;
    let track_x = |t: usize| first_x + t as i32 * track_w;
    let len_x = |t: usize| lens_x + t as i32 * 18;

    let header_y = title_height(p) + 6;
    p.text(8, header_y, "FILA", theme::FAINT)?;
    for (t, track) in project.tracks.iter().enumerate() {
        let max = ((track_w / cw) as usize).saturating_sub(1);
        // Con la fuente grande entran 3 letras: sin el guion, HH-CER y HH-ABI se distinguen.
        let name = if max < 5 {
            track.name.replace('-', "")
        } else {
            track.name.clone()
        };
        let name = fit(&name, max);
        let color = if t == app.track {
            theme::ACCENT
        } else {
            theme::FAINT
        };
        p.text(track_x(t), header_y, &name, color)?;
    }
    p.text_small(lens_x, header_y + ch - 16, "LARGO", theme::FAINT)?;

    let lines = 2;
    let rows_y0 = header_y + ch + 4;
    let bottom = HEIGHT - footer_height(p, lines) - 3 * ch - 10;
    let visible = ((bottom - rows_y0) / ch) as usize;
    // Filas existentes más la nueva.
    let total = (project.song.rows.len() + 1).min(MAX_SONG_ROWS);
    let start = scroll_start(total, row, visible);
    let cursor = cursor_color(view);
    for (i, r) in (start..total.min(start + visible)).enumerate() {
        let y = rows_y0 + i as i32 * ch;
        let current = r == row;
        if current {
            p.fill(0, y, WIDTH, ch, theme::ROW_HI)?;
            p.fill(0, y, 3, ch, cursor)?;
        }
        let playing_here =
            (0..TRACKS).any(|t| app.head(t).is_some_and(|h| usize::from(h.row) == r));
        if playing_here {
            p.text(8, y, ">", theme::GOOD)?;
        }
        let num_color = if current { theme::TEXT } else { theme::DIM };
        p.text(8 + cw, y, &format!("{r:02}"), num_color)?;
        for t in 0..TRACKS {
            let cell = project.chain_id_at(t, r);
            let text = match cell {
                Some(c) if project.chain_uses(c) > 1 => format!("{c:03}*"),
                Some(c) => format!("{c:03}"),
                None => "---".to_string(),
            };
            let x = track_x(t);
            let head = app.head(t).filter(|h| usize::from(h.row) == r);
            if selected(app, t, r) {
                p.fill(x - 2, y, 4 * cw + 4, ch, SELECT)?;
            }
            if current && t == app.track {
                p.text_inverted(x, y, &text, cursor)?;
            } else {
                let color = if head.is_some() {
                    theme::GOOD
                } else if cell.is_some() {
                    theme::TEXT
                } else {
                    theme::FAINT
                };
                p.text(x, y, &text, color)?;
            }
            // La celda que suena: una barra con el avance por la cadena.
            if let Some(h) = head {
                let len = project.chain_at(t, r).map_or(0, |c| c.len()).max(1);
                let w = 3 * cw;
                let done =
                    (w * (i32::from(h.entry) * 16 + i32::from(h.step) + 1)) / (len as i32 * 16);
                p.fill(x, y + ch - 2, w, 2, theme::LINE)?;
                p.fill(x, y + ch - 2, done.min(w), 2, theme::GOOD)?;
            }
        }
        if r < project.song.rows.len() {
            let ty = y + (ch - 16) / 2;
            for (t, len) in project.chain_lengths(r).iter().enumerate() {
                let text = len.map_or("-".to_string(), |l| l.to_string());
                p.text_small(len_x(t), ty, &text, theme::DIM)?;
            }
            if project.row_misaligned(r) {
                p.text_small(WIDTH - 8 - 8, ty, "!", theme::WARN)?;
            }
        }
    }

    let info_y = bottom + 2;
    p.fill(0, info_y, WIDTH, 1, theme::LINE)?;
    let line1 = info_y + 4;
    let align = alignment_text(app, row);
    let color = if align.starts_with('!') {
        theme::WARN
    } else {
        theme::DIM
    };
    p.text(8, line1, &fit(&align, p.cols() - 2), color)?;
    let cell = project.chain_id_at(app.track, row);
    let detail = match cell {
        Some(c) => {
            let uses = project.chain_uses(c);
            let len = project.chain(c).map_or(0, |ch| ch.len());
            let block = project.block_rows(app.track, row);
            format!(
                "CADENA {c:03} ({}, usada {})  bloque {:02}-{:02}: {}",
                plural(len, "frase", "frases"),
                plural(uses, "vez", "veces"),
                block.start,
                block.end - 1,
                duration(app, project.block_phrases(app.track, row))
            )
        }
        None => "celda vacía: A pone la última cadena, 2xA una nueva".to_string(),
    };
    p.text(8, line1 + ch + 2, &fit(&detail, p.cols() - 2), theme::DIM)?;
    message_line(p, line1 + 2 * ch + 4, app, view.now)?;
    footer_or_selection(p, app, lines, &SONG_HELP)
}

/// Cadena de la pista actual en la fila actual: entradas (frase, transposición), cabezal y
/// vista previa de la frase bajo el cursor.
pub fn chain(p: &mut Painter, app: &App, view: &View) -> R {
    let project = &app.project;
    let sec = app.section;
    let track = &project.tracks[app.track];
    let id = project.chain_id_at(app.track, sec.row);
    let id_text = id.map_or("---".to_string(), |c| format!("{c:03}"));
    let uses = id.map_or(String::new(), |c| {
        format!(
            "  (usada {})",
            plural(project.chain_uses(c), "vez", "veces")
        )
    });
    title(
        p,
        app,
        &format!(
            "{id_text}  {} {}  fila {:02}{uses}",
            app.track + 1,
            track.name,
            sec.row
        ),
    )?;
    let (cw, ch) = (p.cw, p.ch);
    let large = app.font == FontSize::Large;
    let x_of = |col: i32| 8 + col * cw;
    let header_y = title_height(p) + 6;
    let (phrase_x, transp_x) = (6, 14);
    p.text(x_of(0), header_y, "POS", theme::FAINT)?;
    let col_color = |c| {
        if app.chain_col == c {
            theme::ACCENT
        } else {
            theme::FAINT
        }
    };
    p.text(
        x_of(phrase_x),
        header_y,
        "FRASE",
        col_color(ChainCol::Phrase),
    )?;
    p.text(
        x_of(transp_x),
        header_y,
        "TRANSP.",
        col_color(ChainCol::Transpose),
    )?;

    let lines = 2;
    let rows_y0 = header_y + ch + 4;
    let bottom = HEIGHT - footer_height(p, lines) - 3 * ch - 10;
    let visible = ((bottom - rows_y0) / ch) as usize;
    let start = scroll_start(CHAIN_LEN, sec.entry, visible);
    let chain = id.and_then(|c| project.chain(c));
    let cursor = cursor_color(view);
    let head = app
        .head(app.track)
        .filter(|h| usize::from(h.row) == sec.row);
    for (i, k) in (start..CHAIN_LEN.min(start + visible)).enumerate() {
        let y = rows_y0 + i as i32 * ch;
        let current = k == sec.entry;
        if current {
            p.fill(0, y, WIDTH, ch, theme::ROW_HI)?;
            p.fill(0, y, 3, ch, cursor)?;
        }
        let on_head = head.is_some_and(|h| usize::from(h.entry) == k);
        if on_head {
            p.text(x_of(0), y, ">", theme::GOOD)?;
        }
        let num_color = if on_head { theme::GOOD } else { theme::DIM };
        p.text(x_of(1), y, &format!("{:02}", k + 1), num_color)?;
        let entry = chain.and_then(|c| c.entry(k));
        let (phrase, transpose) = match entry {
            Some(e) => {
                let shared = if project.phrase_uses(e.phrase) > 1 {
                    "*"
                } else {
                    ""
                };
                (
                    format!("{:03}{shared}", e.phrase),
                    format!("{:+03}", e.transpose),
                )
            }
            None => ("---".to_string(), "---".to_string()),
        };
        let color = if entry.is_some() {
            theme::TEXT
        } else {
            theme::FAINT
        };
        if selected(app, 0, k) {
            let x0 = x_of(phrase_x) - 2;
            p.fill(x0, y, x_of(transp_x + 5) - x0, ch, SELECT)?;
        }
        for (col, x, text) in [
            (ChainCol::Phrase, phrase_x, &phrase),
            (ChainCol::Transpose, transp_x + 1, &transpose),
        ] {
            if current && app.chain_col == col {
                p.text_inverted(x_of(x), y, text, cursor)?;
            } else {
                p.text(x_of(x), y, text, color)?;
            }
        }
        // Avance del cabezal dentro de la frase.
        if let Some(h) = head.filter(|_| on_head) {
            let w = 3 * cw;
            p.fill(x_of(phrase_x), y + ch - 2, w, 2, theme::LINE)?;
            p.fill(
                x_of(phrase_x),
                y + ch - 2,
                w * (i32::from(h.step) + 1) / PHRASE_STEPS as i32,
                2,
                theme::GOOD,
            )?;
        }
    }

    // Vista previa de la frase bajo el cursor (solo con la fuente chica).
    let preview = chain.and_then(|c| c.entry(sec.entry));
    if !large && let Some(e) = preview {
        let px = x_of(26);
        p.text(
            px,
            header_y,
            &format!("FRASE {:03} (vista previa)", e.phrase),
            theme::FAINT,
        )?;
        if let Some(phrase) = project.phrase(e.phrase) {
            let drum = track.kind() == TrackKind::Drum;
            let playing_step = head
                .filter(|h| usize::from(h.entry) == sec.entry)
                .map(|h| usize::from(h.step));
            for (i, s) in phrase.steps.iter().enumerate() {
                let x = px + (i / 8) as i32 * 22 * cw;
                let y = rows_y0 + (i % 8) as i32 * ch;
                let text = match s.note {
                    NoteCell::On(n) => {
                        let chord = if !drum && s.chord.kind != ChordKind::None {
                            s.chord.label()
                        } else {
                            String::new()
                        };
                        format!("{} {chord}", note_name(n))
                    }
                    NoteCell::Off => "OFF".to_string(),
                    NoteCell::Empty => "---".to_string(),
                };
                let num_color = if playing_step == Some(i) {
                    theme::GOOD
                } else {
                    theme::DIM
                };
                p.text(x, y, &format!("{:02}", i + 1), num_color)?;
                let color = if s.is_empty() {
                    theme::FAINT
                } else {
                    theme::track(track.kind())
                };
                p.text(x + 3 * cw, y, &text, color)?;
            }
        }
    }

    let info_y = bottom + 2;
    p.fill(0, info_y, WIDTH, 1, theme::LINE)?;
    let line1 = info_y + 4;
    let len = chain.map_or(0, |c| c.len());
    let text = match id {
        Some(_) => format!(
            "{} = {}; la cadena termina en la primera posición vacía",
            plural(len, "frase", "frases"),
            duration(app, len)
        ),
        None => "sin cadena en esta fila: A en FRASE crea cadena y frase".to_string(),
    };
    p.text(8, line1, &fit(&text, p.cols() - 2), theme::DIM)?;
    let hint = match app.chain_col {
        ChainCol::Phrase => "FRASE  A+izq/der ±1  A+arr/abj ±10  2xA en vacía: frase nueva",
        ChainCol::Transpose => "TRANSP.  A+izq/der ±1  A+arr/abj ±12  A+B: 0",
    };
    p.text(8, line1 + ch + 2, &fit(hint, p.cols() - 2), theme::FAINT)?;
    message_line(p, line1 + 2 * ch + 4, app, view.now)?;
    footer_or_selection(p, app, lines, &CHAIN_HELP)
}

const INSTRUMENT_HELP: [&str; 7] = [
    "A+cruz voces",
    "A+B por defecto",
    "X escuchar",
    "Y copiar instr.",
    "Sel+A pegar instr.",
    "L1/R1 pista",
    "Start frase",
];

/// Instrumento (H4): por ahora solo el tipo y las voces de la pista; el resto llega en H8.
pub fn instrument(p: &mut Painter, app: &App, view: &View) -> R {
    let track = &app.project.tracks[app.track];
    let inst = &track.instrument;
    title(p, app, &format!("PISTA {} {}", app.track + 1, track.name))?;
    let (ch, cw) = (p.ch, p.cw);
    let label_x = 8 + 2 * cw;
    let value_x = 8 + 14 * cw;
    let mut y = title_height(p) + 8;
    // Seleccionado, el instrumento entero: un marco alrededor de sus parámetros.
    if app.selection.is_some() {
        p.outline(2, y - 4, WIDTH - 4, 3 * ch + 14, 3, SELECT)?;
    }

    let kind = match inst.patch {
        Patch::Drum(d) => match d.model {
            DrumModel::Kick => "PERCUSIÓN: BOMBO",
            DrumModel::Snare => "PERCUSIÓN: CAJA",
            DrumModel::ClosedHat => "PERCUSIÓN: HAT CERRADO",
            DrumModel::OpenHat => "PERCUSIÓN: HAT ABIERTO",
            DrumModel::Clap => "PERCUSIÓN: PALMAS",
        },
        Patch::Sampler => "SAMPLE (llega en H7)",
        Patch::Synth(_) => "SUSTRACTIVO",
    };
    p.text(label_x, y, "tipo", theme::DIM)?;
    p.text(value_x, y, kind, theme::TEXT)?;
    y += ch + 6;
    p.text(8, y, "GENERAL", theme::FAINT)?;
    y += ch;
    p.fill(0, y, WIDTH, ch, theme::ROW_HI)?;
    p.text(8, y, ">", cursor_color(view))?;
    p.text(label_x, y, "voces", theme::TEXT)?;
    p.text_inverted(value_x, y, &format!("{}", inst.voices), cursor_color(view))?;
    y += ch + 4;

    let mono = match inst.patch {
        Patch::Synth(_) => "1 = mono: la nota nueva sigue sobre la anterior (legato).",
        _ => "1 = mono: el golpe nuevo corta el anterior (con un fundido de 5 ms).",
    };
    let lines = [
        mono,
        "2 a 8: cada nota suena su largo y su relajación; las siguientes se",
        "superponen. Con todas ocupadas, la nota nueva roba la más vieja de la pista.",
        "",
        "La bolsa tiene 16 voces para las 8 pistas (ADR 0007).",
        "El resto de los parámetros llega con la pantalla completa (H8).",
    ];
    for line in lines {
        p.text(label_x, y, &fit(line, p.cols() - 4), theme::DIM)?;
        y += ch;
    }
    message_line(p, y + 4, app, view.now)?;
    footer_or_selection(p, app, 2, &INSTRUMENT_HELP)
}

/// Mezcla: llega con el motor (H9).
pub fn pending(p: &mut Painter, app: &App, view: &View) -> R {
    let track = &app.project.tracks[app.track];
    title(p, app, &format!("PISTA {} {}", app.track + 1, track.name))?;
    let text = fit("pantalla pendiente: llega con la mezcla (H9)", p.cols() - 2);
    let w = p.text_width(&text);
    p.text((WIDTH - w) / 2, HEIGHT / 2 - p.ch, &text, theme::DIM)?;
    message_line(p, HEIGHT / 2 + p.ch, app, view.now)?;
    footer(p, app.screen, 2, &NAV_HELP)
}

const BUTTON_LABELS: [(Button, &str); 15] = [
    (Button::Up, "ARR"),
    (Button::Down, "ABJ"),
    (Button::Left, "IZQ"),
    (Button::Right, "DER"),
    (Button::A, "A"),
    (Button::B, "B"),
    (Button::X, "X"),
    (Button::Y, "Y"),
    (Button::L1, "L1"),
    (Button::R1, "R1"),
    (Button::L2, "L2"),
    (Button::R2, "R2"),
    (Button::Select, "SEL"),
    (Button::Start, "STA"),
    (Button::Fn, "FN"),
];

/// Proyecto: archivo (H6), ajustes y, si entra, la prueba de controles y la muestra de fuentes.
pub fn project(p: &mut Painter, app: &App, view: &View) -> R {
    title(p, app, "")?;
    let ch = p.ch;
    let cw = p.cw;
    let cursor = cursor_color(view);
    let label_x = 8 + 2 * cw;
    let value_x = 8 + 22 * cw;
    let lines = 2;
    let bottom = HEIGHT - footer_height(p, lines) - ch - 2;
    let mut y = title_height(p) + 8;

    let item_row = |p: &mut Painter, y: i32, item: ProjectItem| -> R {
        let selected = ProjectItem::ALL[app.project_item] == item;
        let (label, value) = match item {
            ProjectItem::Name => ("nombre", format!("[{}]", app.project.name)),
            ProjectItem::Tempo => ("tempo", format!("{:.1} BPM", app.project.tempo_bpm)),
            ProjectItem::Swing => ("swing", swing_text(app.project.swing)),
            ProjectItem::Save => ("GUARDAR", String::new()),
            ProjectItem::SaveAs => ("GUARDAR COMO...", String::new()),
            ProjectItem::Load => ("CARGAR...", String::new()),
            ProjectItem::New => ("NUEVO", String::new()),
            ProjectItem::Font => (
                "fuente",
                match app.font {
                    FontSize::Small => "8×16",
                    FontSize::Large => "12×24",
                }
                .to_string(),
            ),
            ProjectItem::AuditionOnEdit => (
                "audición al editar",
                if app.audition_on_edit { "SÍ" } else { "NO" }.to_string(),
            ),
            ProjectItem::Follow => (
                "seguir reproducción",
                if app.follow { "SÍ" } else { "NO" }.to_string(),
            ),
            ProjectItem::Quit => ("SALIR", String::new()),
        };
        if selected {
            p.fill(0, y, WIDTH, ch, theme::ROW_HI)?;
            p.text(8, y, ">", cursor)?;
        }
        let label_color = match item {
            ProjectItem::Quit => theme::WARN,
            ProjectItem::Name => theme::DIM,
            _ => theme::TEXT,
        };
        if selected && value.is_empty() {
            p.text_inverted(label_x, y, label, cursor)?;
        } else {
            p.text(label_x, y, label, label_color)?;
        }
        if !value.is_empty() {
            let end = if selected {
                p.text_inverted(value_x, y, &value, cursor)? + 2
            } else {
                p.text(value_x, y, &value, theme::ACCENT)?
            };
            if item == ProjectItem::Name {
                let state = match (app.modified, app.read_only) {
                    (_, true) => "solo lectura",
                    (true, false) => "* sin guardar",
                    (false, false) => "",
                };
                let cols = ((WIDTH - end) / cw - 2).max(0) as usize;
                p.text(end + cw, y, &fit(state, cols), theme::WARN)?;
            }
        }
        Ok(())
    };

    for item in [ProjectItem::Name, ProjectItem::Tempo, ProjectItem::Swing] {
        item_row(p, y, item)?;
        y += ch;
    }
    let data = app
        .data
        .as_ref()
        .map_or("(sin carpeta de datos)".to_string(), |d| {
            home_relative(d.root())
        });
    let info = [
        ("octava base", format!("{}", app.octave)),
        ("CPU de audio", load_text(app)),
        ("datos", data),
    ];
    let value_cols = ((WIDTH - value_x) / cw - 1) as usize;
    for (label, value) in info {
        p.text(label_x, y, label, theme::DIM)?;
        p.text(value_x, y, &fit_path(&value, value_cols), theme::TEXT)?;
        y += ch;
    }
    let groups: [&[ProjectItem]; 3] = [
        &[
            ProjectItem::Save,
            ProjectItem::SaveAs,
            ProjectItem::Load,
            ProjectItem::New,
        ],
        &[
            ProjectItem::Font,
            ProjectItem::AuditionOnEdit,
            ProjectItem::Follow,
        ],
        &[ProjectItem::Quit],
    ];
    for group in groups {
        y += 6;
        for &item in group {
            item_row(p, y, item)?;
            y += ch;
        }
    }

    // Prueba de controles: sirve para ver el mapeo en la consola. Etiquetas con la fuente chica.
    // Con la fuente grande no entra y se omite.
    let (bw, bh, gap) = (34, 18, 6);
    let size = 44;
    if y + 8 + ch + 4 + bh + 8 + size + 10 <= bottom {
        y += 8;
        p.text(8, y, "PRUEBA DE CONTROLES", theme::FAINT)?;
        y += ch + 4;
        for (i, (button, label)) in BUTTON_LABELS.iter().enumerate() {
            let x = 8 + i as i32 * (bw + gap);
            let held = view.is_held(*button);
            if held {
                p.fill(x, y, bw, bh, theme::ACCENT)?;
            } else {
                p.outline(x, y, bw, bh, 1, theme::LINE)?;
            }
            let lw = label.chars().count() as i32 * 8;
            let color = if held { theme::BG } else { theme::DIM };
            p.text_small(x + (bw - lw) / 2, y + 1, label, color)?;
        }
        y += bh + 8;
        for (i, name) in ["stick izq", "stick der"].iter().enumerate() {
            let x = 8 + i as i32 * 200;
            let (sx, sy) = view.sticks[i];
            p.outline(x, y, size, size, 1, theme::LINE)?;
            p.fill(x + size / 2, y + 2, 1, size - 4, theme::STEP_EMPTY)?;
            p.fill(x + 2, y + size / 2, size - 4, 1, theme::STEP_EMPTY)?;
            let dx = x + size / 2 + (sx * (size / 2 - 4) as f32) as i32;
            let dy = y + size / 2 - (sy * (size / 2 - 4) as f32) as i32;
            p.fill(dx - 3, dy - 3, 6, 6, theme::ACCENT)?;
            p.text_small(x + size + 8, y + 4, name, theme::DIM)?;
            p.text_small(
                x + size + 8,
                y + 22,
                &format!("{sx:+.2} {sy:+.2}"),
                theme::TEXT,
            )?;
        }
        y += size + 10;
    }

    // Muestra de las dos fuentes para comparar la legibilidad en la consola (si entra).
    if y + 16 + 4 + 24 <= bottom {
        let sample = "BOMBO C#4 may7 vel 100 ¿Sección añadida? 0123456789";
        p.text_small(8, y, "8×16 ", theme::FAINT)?;
        p.text_small(8 + 6 * 8, y, sample, theme::TEXT)?;
        y += 16 + 4;
        p.fonts.large.draw(p.canvas, 8, y, "12×24", theme::FAINT)?;
        p.fonts
            .large
            .draw(p.canvas, 8 + 6 * 12, y, &fit(sample, 46), theme::TEXT)?;
    }

    message_line(p, bottom, app, view.now)?;
    let help: &[&str] = match ProjectItem::ALL[app.project_item] {
        ProjectItem::Tempo => &["A+izq/der ±0,1", "A+arr/abj ±1", "A+B 120", "B volver"],
        ProjectItem::Swing => &["A+izq/der ±1", "A+arr/abj ±5", "A+B recto", "B volver"],
        item if item.is_action() => &[
            "A ejecutar",
            "B volver",
            "Sel+abajo volver",
            "Sel+Start(1s) salir",
        ],
        _ => &[
            "A cambiar",
            "B volver",
            "Sel+abajo volver",
            "Sel+Start(1s) salir",
        ],
    };
    footer(p, app.screen, lines, help)
}

/// Swing con su nombre en los valores de referencia (spec-motor, "Swing").
fn swing_text(swing: u8) -> String {
    let name = match swing {
        50 => " (recto)",
        66 | 67 => " (tresillo)",
        75 => " (medio paso)",
        _ => "",
    };
    format!("{swing} %{name}")
}

/// La carpeta con `~` en lugar del home, como la escribiría el usuario.
pub fn home_relative(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && text.starts_with(&home) => {
            format!("~{}", &text[home.len()..])
        }
        _ => text,
    }
}

/// Recorta una ruta por el principio (lo importante es el final).
pub fn fit_path(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        let tail: String = s.chars().skip(n + 1 - max).collect();
        format!("…{tail}")
    }
}
