//! Pantalla Frase (spec-ui §4): la frase de la pista actual en la sección, paso por paso.

use super::selection::{SELECT, footer_or_selection, selected};
use super::{
    HEIGHT, Painter, R, View, WIDTH, cursor_color, fit, footer_height, message_line, status_parts,
    theme, title_bar, title_height,
};
use crate::app::{App, FontSize, PhraseCol};
use crate::model::{ChordKind, NoteCell, PHRASE_STEPS, TrackKind, gate_label, note_name};

pub(super) const HELP: [&str; 10] = [
    "Start frase",
    "Sel+Start loop",
    "A poner",
    "A+B borrar",
    "A+L1/R1 acorde",
    "A+Y clonar",
    "X escuchar",
    "Y selec.",
    "Sel+A pegar",
    "L2/R2 frase",
];

/// Columna (en caracteres) de cada campo: PASO, NOTA, ACORDE, VEL, LARGO.
const COL_STEP: i32 = 1;
const COLS: [(PhraseCol, &str, i32, usize); 4] = [
    (PhraseCol::Note, "NOTA", 6, 3),
    (PhraseCol::Chord, "ACORDE", 12, 6),
    (PhraseCol::Velocity, "VEL", 20, 3),
    (PhraseCol::Gate, "LARGO", 25, 3),
];
/// Columnas de v1, solo con la fuente chica y deshabilitadas.
const V1_COLS: [(&str, i32, &str); 5] = [
    ("LOCK 1", 32, "---- ---"),
    ("LOCK 2", 42, "---- ---"),
    ("COND", 52, "---"),
    ("MICRO", 59, "---"),
    ("REP", 66, "-"),
];

pub fn draw(p: &mut Painter, app: &App, view: &View) -> R {
    let project = &app.project;
    let sec = app.section;
    let track = &project.tracks[app.track];
    let chain = project.chain_id_at(app.track, sec.row);
    let phrase_id = project.phrase_id_at(app.track, sec);
    let phrase = phrase_id.and_then(|id| project.phrase(id));
    let large = app.font == FontSize::Large;

    let id = phrase_id.map_or("---".to_string(), |id| format!("{id:03}"));
    let chain_text = chain.map_or("sin cadena".to_string(), |c| {
        format!("cadena {c:03} pos {:02}", sec.entry + 1)
    });
    let parts = status_parts(app);
    title_bar(
        p,
        app.screen,
        &format!(
            "{id}  {} {}  {chain_text}  fila {:02}",
            app.track + 1,
            track.name,
            sec.row
        ),
        &parts,
    )?;

    let (cw, ch) = (p.cw, p.ch);
    let x_of = |col: i32| 8 + col * cw;
    let header_y = title_height(p) + 4;
    let rows_y0 = header_y + ch + 2;
    let cursor = cursor_color(view);

    p.text(x_of(COL_STEP - 1), header_y, "PASO", theme::FAINT)?;
    for (col, name, x, _) in COLS {
        let color = if col == app.phrase_col {
            theme::ACCENT
        } else {
            theme::FAINT
        };
        p.text(x_of(x), header_y, name, color)?;
    }
    if !large {
        for (name, x, _) in V1_COLS {
            p.text(x_of(x), header_y, name, theme::LINE)?;
        }
    }

    let kind_color = theme::track(track.kind());
    let playing = app.playing_step(app.track);
    for i in 0..PHRASE_STEPS {
        let y = rows_y0 + i as i32 * ch;
        let on_row = i == app.phrase_row;
        let on_head = playing == Some(i);
        if on_row {
            p.fill(0, y, WIDTH, ch, theme::ROW_HI)?;
            p.fill(0, y, 3, ch, cursor)?;
        } else if i % 4 == 0 {
            p.fill(0, y, WIDTH, ch, theme::PANEL)?;
        }
        let num_color = if on_head {
            theme::GOOD
        } else if on_row || i % 4 == 0 {
            theme::TEXT
        } else {
            theme::DIM
        };
        if on_head {
            p.text(x_of(COL_STEP - 1) - 4, y, ">", theme::GOOD)?;
        }
        p.text(x_of(COL_STEP), y, &format!("{:02}", i + 1), num_color)?;

        let step = phrase.map(|ph| ph.steps[i]);
        let has_note = step.is_some_and(|s| s.root().is_some());
        for (col, _, x, width) in COLS {
            let (text, color) = match (col, step) {
                (PhraseCol::Note, Some(s)) => match s.note {
                    NoteCell::On(n) => (note_name(n), kind_color),
                    NoteCell::Off => ("OFF".to_string(), theme::WARN),
                    NoteCell::Empty => ("---".to_string(), theme::FAINT),
                },
                (PhraseCol::Chord, Some(s))
                    if has_note
                        && track.kind() != TrackKind::Drum
                        && s.chord.kind != ChordKind::None =>
                {
                    (s.chord.label(), theme::TEXT)
                }
                (PhraseCol::Velocity, Some(s)) if has_note => {
                    (format!("{:03}", s.velocity), theme::TEXT)
                }
                (PhraseCol::Gate, Some(s)) if has_note => (gate_label(s.gate_ticks), theme::TEXT),
                _ => ("---".to_string(), theme::FAINT),
            };
            if selected(app, col.index(), i) {
                p.fill(x_of(x) - 2, y, (width as i32) * cw + 4, ch, SELECT)?;
            }
            if on_row && col == app.phrase_col {
                let padded = format!("{text:<width$}");
                p.text_inverted(x_of(x), y, &padded, cursor)?;
            } else {
                p.text(x_of(x), y, &text, color)?;
            }
        }
        if !large {
            for (_, x, placeholder) in V1_COLS {
                p.text(x_of(x), y, placeholder, theme::LINE)?;
            }
        }
    }

    let context = column_context(app);
    if large {
        // Con 12×24 entran las 16 filas y una sola línea de pie: el mensaje o el contexto.
        let lines = 1;
        debug_assert!(rows_y0 + 16 * ch <= HEIGHT - footer_height(p, lines));
        footer_or_selection(p, app, lines, &[])?;
        if app.selection.is_some() {
            return Ok(());
        }
        let y = HEIGHT - footer_height(p, lines) + 4;
        if app.message_at(view.now).is_some() {
            message_line(p, y, app, view.now)?;
        } else {
            let max = ((WIDTH - 140) / cw) as usize;
            p.text(8, y, &fit(&context, max), theme::DIM)?;
        }
        return Ok(());
    }

    let info_y = rows_y0 + PHRASE_STEPS as i32 * ch + 4;
    p.fill(0, info_y, WIDTH, 1, theme::LINE)?;
    let line1 = info_y + 5;
    p.text(8, line1, &fit(&context, p.cols() - 2), theme::DIM)?;
    let hint = "MVP: NOTA ACORDE VEL LARGO   v1: LOCK..REP";
    p.text(8, line1 + ch + 2, hint, theme::FAINT)?;
    message_line(p, line1 + 2 * ch + 6, app, view.now)?;
    let lines = 2;
    footer_or_selection(p, app, lines, &HELP)
}

/// Descripción de la columna bajo el cursor y de sus controles.
fn column_context(app: &App) -> String {
    let project = &app.project;
    let Some(step) = project.step(app.track, app.section, app.phrase_row) else {
        return "sin frase en esta sección: A en NOTA crea cadena y frase".to_string();
    };
    let drum = app.track_kind() == TrackKind::Drum;
    match (app.phrase_col, step.note) {
        (PhraseCol::Note, NoteCell::On(n)) => {
            format!("NOTA {}   A+izq/der ±1   A+arr/abj ±12", note_name(n))
        }
        (PhraseCol::Note, _) => "NOTA vacía   A: poner la última nota de la pista".to_string(),
        (_, NoteCell::Empty | NoteCell::Off) => "paso sin nota".to_string(),
        (PhraseCol::Chord, _) if drum => "ACORDE: se ignora en percusión".to_string(),
        (PhraseCol::Chord, NoteCell::On(n)) => {
            let notes: Vec<String> = step
                .chord
                .notes(n)
                .as_slice()
                .iter()
                .map(|&m| note_name(m))
                .collect();
            format!(
                "ACORDE {}: {}   A+izq/der tipo   A+arr/abj inversión",
                step.chord.kind.label(),
                notes.join(" ")
            )
        }
        (PhraseCol::Velocity, _) => {
            format!("VEL {:03}   A+izq/der ±1   A+arr/abj ±10", step.velocity)
        }
        (PhraseCol::Gate, _) => format!(
            "LARGO {} (pasos)   A+izq/der ±1   A+arr/abj ±4",
            gate_label(step.gate_ticks)
        ),
    }
}
