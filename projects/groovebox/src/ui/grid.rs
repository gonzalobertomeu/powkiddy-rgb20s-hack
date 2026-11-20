//! Pantalla Grilla (spec-ui §2): las 8 pistas en la sección actual, 16 pasos cada una.

use sdl2::pixels::Color;

use super::{
    HEIGHT, Painter, R, View, WIDTH, cursor_color, footer, footer_height, message_line,
    status_parts, theme, title_bar, title_height,
};
use crate::app::{App, GridCol};
use crate::model::{
    ChordKind, GATE_INF, NoteCell, PHRASE_STEPS, Phrase, TICKS_PER_STEP, TRACKS, TrackKind,
    note_name,
};

const ROW_H: i32 = 32;
const BLOCK_H: i32 = 20;
const GROUP_GAP: i32 = 6;

const HELP: [&str; 8] = [
    "A poner/editar",
    "A+B borrar",
    "A+L1/R1 acorde",
    "X escuchar",
    "L2/R2 sección",
    "Start canción",
    "Sel+Start loop",
    "Sel+cruz pantalla",
];

struct Layout {
    label_x: i32,
    fra_x: i32,
    steps_x0: i32,
    pitch: i32,
    block_w: i32,
    flags_x: i32,
}

impl Layout {
    fn new(cw: i32) -> Self {
        let label_x = 8;
        // "1 BOMBO  " ocupa 9 columnas; FRA, 3 dígitos y la marca de compartida.
        let fra_x = label_x + 9 * cw;
        let steps_x0 = fra_x + 4 * cw + 6;
        let flags_x = WIDTH - 8 - 2 * cw;
        let pitch = (flags_x - 4 - steps_x0 - 3 * GROUP_GAP) / PHRASE_STEPS as i32;
        Self {
            label_x,
            fra_x,
            steps_x0,
            pitch,
            block_w: pitch - 4,
            flags_x,
        }
    }

    fn step_x(&self, i: usize) -> i32 {
        self.steps_x0 + i as i32 * self.pitch + (i / 4) as i32 * GROUP_GAP
    }
}

/// Último paso que alcanza la nota del paso `i` (por su largo, hasta la próxima nota).
fn gate_end(phrase: &Phrase, i: usize) -> usize {
    let next = (i + 1..PHRASE_STEPS)
        .find(|&j| !phrase.steps[j].is_empty())
        .unwrap_or(PHRASE_STEPS);
    let ticks = phrase.steps[i].gate_ticks;
    let span = if ticks == GATE_INF {
        PHRASE_STEPS
    } else {
        usize::from(ticks.div_ceil(TICKS_PER_STEP))
    };
    (i + span.max(1)).min(next) - 1
}

pub fn draw(p: &mut Painter, app: &App, view: &View) -> R {
    let project = &app.project;
    let sec = app.section;
    let count = project.section_count(sec.row);
    let section = if sec.entry < count {
        format!("sección {}/{count}", sec.entry + 1)
    } else {
        format!("sección {} (nueva)", sec.entry + 1)
    };
    let row_label = if sec.row < project.song.rows.len() {
        format!("fila {:02}", sec.row)
    } else {
        format!("fila {:02} (nueva)", sec.row)
    };
    let parts = status_parts(app);
    title_bar(p, app.screen, &format!("{row_label}  {section}"), &parts)?;

    let l = Layout::new(p.cw);
    let (cw, ch) = (p.cw, p.ch);
    let header_y = title_height(p) + 6;
    let rows_y0 = header_y + ch + 6;
    let cursor = cursor_color(view);

    // Encabezado: número de paso, más fuerte en los tiempos (1, 5, 9, 13) y en la columna del cursor.
    p.text(l.label_x, header_y, "#", theme::FAINT)?;
    p.text(l.label_x + 2 * cw, header_y, "PISTA", theme::FAINT)?;
    let fra_color = if app.grid_col == GridCol::Phrase {
        theme::ACCENT
    } else {
        theme::FAINT
    };
    p.text(l.fra_x, header_y, "FRA", fra_color)?;
    // Con la fuente grande los números de dos cifras no entran: solo tiempos, cursor y cabezal.
    let sparse = p.text_width("16") + 6 > l.pitch;
    let playing = (0..TRACKS).find_map(|t| app.playing_step(t));
    for i in 0..PHRASE_STEPS {
        let n = format!("{}", i + 1);
        let on_cursor = app.grid_col == GridCol::Step(i);
        let on_head = playing == Some(i);
        if sparse && i % 4 != 0 && !on_cursor && !on_head {
            continue;
        }
        let color = if on_head {
            theme::GOOD
        } else if on_cursor {
            theme::ACCENT
        } else if i % 4 == 0 {
            theme::DIM
        } else {
            theme::FAINT
        };
        let x = l.step_x(i) + (l.block_w - p.text_width(&n)) / 2;
        p.text(x, header_y, &n, color)?;
    }

    for t in 0..TRACKS {
        let y = rows_y0 + t as i32 * ROW_H;
        let track = &project.tracks[t];
        let on_row = t == app.track;
        let dim = if track.mix.mute { 0.55 } else { 0.0 };
        let fade = |c: Color| theme::mix(c, theme::BG, dim);
        let text_y = y + (ROW_H - ch) / 2;
        let block_y = y + (ROW_H - BLOCK_H) / 2;

        if on_row {
            p.fill(0, y, WIDTH, ROW_H, theme::ROW_HI)?;
            p.fill(0, y, 3, ROW_H, cursor)?;
        }
        let num_color = if on_row { theme::TEXT } else { theme::FAINT };
        p.text(l.label_x, text_y, &format!("{}", t + 1), num_color)?;
        p.text(
            l.label_x + 2 * cw,
            text_y,
            &track.name,
            fade(theme::track(track.kind())),
        )?;

        // Siguiendo la reproducción, cada pista muestra la sección donde está su cabezal.
        let shown = app.shown_section(t);
        let elsewhere = shown != sec;
        let phrase_id = project.phrase_id_at(t, shown);
        let fra = match phrase_id {
            Some(id) if project.phrase_uses(id) > 1 => format!("{id:03}*"),
            Some(id) => format!("{id:03}"),
            None => "---".to_string(),
        };
        if on_row && app.grid_col == GridCol::Phrase {
            p.text_inverted(l.fra_x, text_y, &fra, cursor)?;
        } else {
            let color = if elsewhere {
                theme::GOOD
            } else if phrase_id.is_some() {
                theme::TEXT
            } else {
                theme::FAINT
            };
            p.text(l.fra_x, text_y, &fra, fade(color))?;
        }

        let phrase = phrase_id.and_then(|id| project.phrase(id));
        match phrase {
            Some(phrase) => draw_steps(p, &l, block_y, phrase, track.kind(), dim)?,
            None if on_row => {
                for i in 0..PHRASE_STEPS {
                    p.outline(
                        l.step_x(i),
                        block_y,
                        l.block_w,
                        BLOCK_H,
                        1,
                        theme::STEP_EMPTY,
                    )?;
                }
            }
            None => {
                let hint = "(sin frase: A en un paso la crea)";
                let max = ((l.flags_x - l.steps_x0) / cw) as usize;
                p.text(l.steps_x0, text_y, &super::fit(hint, max), theme::FAINT)?;
            }
        }

        // Cabezal: una barra bajo el paso que suena (aunque la pista no tenga frase acá).
        if let Some(i) = app.playing_step(t) {
            let color = if track.mix.mute {
                theme::DIM
            } else {
                theme::GOOD
            };
            // Arriba y abajo del bloque, por fuera del marco del cursor.
            p.fill(l.step_x(i), block_y - 6, l.block_w, 2, color)?;
            p.fill(l.step_x(i), block_y + BLOCK_H + 4, l.block_w, 2, color)?;
        }

        if on_row && let GridCol::Step(i) = app.grid_col {
            p.outline(
                l.step_x(i) - 3,
                block_y - 3,
                l.block_w + 6,
                BLOCK_H + 6,
                2,
                cursor,
            )?;
        }

        let mut fx = l.flags_x;
        if track.mix.mute {
            fx = p.text(fx, text_y, "M", theme::WARN)?;
        }
        if track.mix.solo {
            p.text(fx, text_y, "S", theme::GOOD)?;
        }
    }

    // Panel de información de la pista y el paso bajo el cursor.
    let info_y = rows_y0 + TRACKS as i32 * ROW_H + 4;
    p.fill(0, info_y, WIDTH, 1, theme::LINE)?;
    let line1 = info_y + 5;
    let line2 = line1 + ch + 2;
    let track = &project.tracks[app.track];
    let chain = project.chain_id_at(app.track, sec.row);
    let phrase_id = project.phrase_id_at(app.track, sec);
    let chain_text = chain.map_or("sin cadena".to_string(), |c| format!("cadena {c:03}"));
    let phrase_text = match phrase_id {
        Some(id) => {
            let uses = project.phrase_uses(id);
            let veces = if uses == 1 { "vez" } else { "veces" };
            format!("frase {id:03} (usada {uses} {veces})")
        }
        None => "sin frase".to_string(),
    };
    let x = p.text(
        8,
        line1,
        &format!("PISTA {} {}", app.track + 1, track.name),
        theme::track(track.kind()),
    )?;
    let rest = format!("  {chain_text}  {phrase_text}");
    let max = p.cols().saturating_sub(((x + 8) / cw) as usize);
    p.text(x, line1, &super::fit(&rest, max), theme::DIM)?;

    match app.grid_col {
        GridCol::Step(i) => {
            let step = project.step(app.track, sec, i);
            let x = p.text(8, line2, &format!("PASO {:02}", i + 1), theme::TEXT)?;
            let detail = match step.map(|s| (s.note, s)) {
                Some((NoteCell::On(n), s)) => {
                    let chord =
                        if track.kind() != TrackKind::Drum && s.chord.kind != ChordKind::None {
                            format!(" {}", s.chord.label())
                        } else {
                            String::new()
                        };
                    format!(
                        "  {}{chord}  vel {:03}  largo {}",
                        note_name(n),
                        s.velocity,
                        crate::model::gate_label(s.gate_ticks)
                    )
                }
                Some((NoteCell::Off, _)) => "  OFF".to_string(),
                Some((NoteCell::Empty, _)) => "  vacío".to_string(),
                None => "  A crea la frase en esta sección".to_string(),
            };
            p.text(x, line2, &detail, theme::DIM)?;
        }
        GridCol::Phrase => {
            p.spans(
                8,
                line2,
                &[
                    ("FRA", theme::TEXT),
                    ("  A+cruz ±1/±10  2xA nueva  A+Y clonar", theme::DIM),
                ],
            )?;
        }
    }
    let line3 = line2 + ch + 4;
    if app.message_at(view.now).is_some() {
        message_line(p, line3, app, view.now)?;
    } else {
        let elsewhere: Vec<String> = (0..TRACKS)
            .filter(|&t| app.shown_section(t) != sec)
            .map(|t| {
                let s = app.shown_section(t);
                let name = super::fit(&project.tracks[t].name, 5);
                format!("{name} fila {:02} sec. {}", s.row, s.entry + 1)
            })
            .collect();
        if !elsewhere.is_empty() {
            let text = format!("en otra sección: {}", elsewhere.join("  "));
            p.text(8, line3, &super::fit(&text, p.cols() - 2), theme::GOOD)?;
        }
    }

    let lines = 2;
    debug_assert!(line2 + 2 * ch + 4 <= HEIGHT - footer_height(p, lines));
    footer(p, app.screen, lines, &HELP)
}

fn draw_steps(
    p: &mut Painter,
    l: &Layout,
    block_y: i32,
    phrase: &Phrase,
    kind: TrackKind,
    dim: f32,
) -> R {
    let base = theme::track(kind);
    let fade = |c: Color| theme::mix(c, theme::BG, dim);

    for i in 0..PHRASE_STEPS {
        let bg = if i % 4 == 0 {
            theme::STEP_BEAT
        } else {
            theme::STEP_EMPTY
        };
        p.fill(l.step_x(i), block_y, l.block_w, BLOCK_H, fade(bg))?;
    }

    // Largo: una barra que sigue desde el bloque hasta donde llega la nota.
    let bar_h = 6;
    let bar_y = block_y + (BLOCK_H - bar_h) / 2;
    for (i, step) in phrase.steps.iter().enumerate() {
        if step.root().is_none() {
            continue;
        }
        let end = gate_end(phrase, i);
        if end > i {
            let x0 = l.step_x(i) + l.block_w;
            let x1 = l.step_x(end) + l.block_w;
            p.fill(
                x0,
                bar_y,
                x1 - x0,
                bar_h,
                fade(theme::mix(base, theme::BG, 0.45)),
            )?;
        }
    }

    for (i, step) in phrase.steps.iter().enumerate() {
        let x = l.step_x(i);
        match step.note {
            NoteCell::On(_) => {
                let t = 0.35 + 0.65 * f32::from(step.velocity) / 127.0;
                let color = fade(theme::mix(theme::STEP_EMPTY, base, t));
                p.fill(x, block_y, l.block_w, BLOCK_H, color)?;
                // Acorde: rayas horizontales.
                if kind != TrackKind::Drum && step.chord.kind != ChordKind::None {
                    let stripe = fade(theme::BG);
                    p.fill(x, block_y + BLOCK_H / 3 - 1, l.block_w, 2, stripe)?;
                    p.fill(x, block_y + 2 * BLOCK_H / 3, l.block_w, 2, stripe)?;
                }
            }
            NoteCell::Off => {
                let c = fade(theme::WARN);
                let (x0, y0) = (x + l.block_w / 2 - 5, block_y + BLOCK_H / 2 - 5);
                p.line(x0, y0, x0 + 10, y0 + 10, c)?;
                p.line(x0, y0 + 10, x0 + 10, y0, c)?;
            }
            NoteCell::Empty => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Step;

    fn phrase_with(notes: &[(usize, u16)]) -> Phrase {
        let mut p = Phrase::default();
        for &(i, gate) in notes {
            p.steps[i] = Step {
                note: NoteCell::On(60),
                gate_ticks: gate,
                ..Step::default()
            };
        }
        p
    }

    #[test]
    fn gate_bar_stops_at_next_note() {
        let p = phrase_with(&[(0, 96), (2, 24), (8, GATE_INF), (12, 6)]);
        assert_eq!(gate_end(&p, 0), 1);
        assert_eq!(gate_end(&p, 2), 2);
        assert_eq!(gate_end(&p, 8), 11);
        assert_eq!(gate_end(&p, 12), 12);
        let p = phrase_with(&[(14, GATE_INF)]);
        assert_eq!(gate_end(&p, 14), 15);
    }

    #[test]
    fn layout_fits_both_fonts() {
        for cw in [8, 12] {
            let l = Layout::new(cw);
            assert!(l.block_w >= 20, "bloques de al menos 20 px con cw={cw}");
            assert!(l.step_x(15) + l.block_w < l.flags_x);
        }
    }
}
