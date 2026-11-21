//! Mezcla (H9, spec-ui "6. Mezcla"): los 8 canales y el master (medidores, voces, volumen,
//! paneo, envíos, mute y solo) y abajo los efectos y el master. Con 12×24 los canales se
//! angostan a 5 columnas y las filas de efectos muestran solo los valores (la línea de
//! información nombra el parámetro del cursor).

use super::{
    HEIGHT, Painter, R, View, WIDTH, cursor_color, fit, footer, footer_height, message_line,
    status_parts, theme, title_bar, title_height,
};
use crate::app::{App, FontSize};
use crate::engine::pool::POOL_VOICES;
use crate::mixer::{MixRow, channel_value, fx_value, master_gain_label, meter_fraction};
use crate::model::TRACKS;

const HELP: [&str; 9] = [
    "Start canción",
    "Sel+Start loop",
    "A+cruz cambiar",
    "A alternar",
    "A+B defecto",
    "X escuchar",
    "Sel+X/Y mute/solo",
    "L1/R1 canal",
    "Sel+arr volver",
];

/// Arriba de esto el medidor se pinta de rojo (cerca del techo).
const HOT: f32 = 0.891; // −1 dBFS

pub fn draw(p: &mut Painter, app: &App, view: &View) -> R {
    let track = &app.project.tracks[app.track];
    let context = if app.mix.master {
        "MASTER".to_string()
    } else {
        format!("PISTA {} {}", app.track + 1, track.name)
    };
    let parts = status_parts(app);
    title_bar(p, app.screen, &context, &parts)?;

    let large = app.font == FontSize::Large;
    let (cw, ch) = (p.cw, p.ch);
    // 8×16: 8 + 9 × 8 = 80 columnas, como el mockup; 12×24: 5 + 9 × 5 = 50 de 53.
    let label_cols = if large { 5 } else { 8 };
    let col_cols = if large { 5 } else { 8 };
    let label = |r: &str| -> String {
        if large {
            match r {
                "volumen" => "vol",
                "paneo" => "pan",
                "delay" => "dly",
                "reverb" => "rev",
                "nivel" => "niv",
                "voces" => "voc",
                other => other,
            }
            .to_string()
        } else {
            r.to_string()
        }
    };
    let x0 = 8;
    let col_x = |c: usize| x0 + (label_cols + c * col_cols) as i32 * cw;
    let cursor = cursor_color(view);
    let mut y = title_height(p) + 6;

    // Encabezados: número y nombre de cada pista (con 12×24, solo 4 letras) y MASTER.
    for (t, tr) in app.project.tracks.iter().enumerate() {
        let name: String = if large {
            tr.name.chars().take(col_cols - 1).collect()
        } else {
            let short: String = tr.name.chars().take(col_cols - 3).collect();
            format!("{} {short}", t + 1)
        };
        let color = if t == app.track && !app.mix.master {
            theme::ACCENT
        } else {
            theme::track(tr.kind())
        };
        p.text(col_x(t), y, &name, color)?;
    }
    let master_label = if large { "MST" } else { "MASTER" };
    let master_color = if app.mix.master {
        theme::ACCENT
    } else {
        theme::TEXT
    };
    p.text(col_x(TRACKS), y, master_label, master_color)?;
    y += ch + 2;

    // Nivel: barras de pico con caída (−48 a 0 dBFS).
    p.text(x0, y, &label("nivel"), theme::DIM)?;
    let bar_w = (col_cols as i32 - 1) * cw;
    for c in 0..=TRACKS {
        let level = app.meters.level[c];
        let x = col_x(c);
        p.fill(x, y + 3, bar_w, ch - 6, theme::STEP_EMPTY)?;
        let w = (meter_fraction(level) * bar_w as f32).round() as i32;
        let color = if level >= HOT {
            theme::WARN
        } else if c == TRACKS {
            theme::GOOD
        } else {
            theme::track(app.project.tracks[c].kind())
        };
        p.fill(x, y + 3, w, ch - 6, color)?;
    }
    y += ch;

    // Voces en uso / límite (de la pista; de la bolsa en MASTER).
    p.text(x0, y, &label("voces"), theme::DIM)?;
    for (t, tr) in app.project.tracks.iter().enumerate() {
        let used = app.meters.voices[t];
        let color = if used > 0 { theme::TEXT } else { theme::FAINT };
        p.text(
            col_x(t),
            y,
            &format!("{used}/{}", tr.instrument.voices),
            color,
        )?;
    }
    p.text(
        col_x(TRACKS),
        y,
        &format!("{}/{POOL_VOICES}", app.meters.pool_voices),
        theme::TEXT,
    )?;
    y += ch + 2;

    // Filas de canal.
    for (i, &r) in MixRow::ALL
        .iter()
        .enumerate()
        .filter(|(_, r)| r.is_channel())
    {
        let selected_row = app.mix.row == i;
        if selected_row {
            p.fill(0, y, WIDTH, ch, theme::ROW_HI)?;
            p.text(x0 - 6, y, ">", cursor)?;
        }
        p.text(x0 + cw / 2, y, &label(r.label()), theme::TEXT)?;
        for (t, tr) in app.project.tracks.iter().enumerate() {
            let value = channel_value(&tr.mix, r);
            let on_cursor = selected_row && t == app.track && !app.mix.master;
            let x = col_x(t) + cw;
            if on_cursor {
                p.text_inverted(x, y, &value, cursor)?;
            } else {
                let color = match (r, value.as_str()) {
                    (MixRow::Mute, "M") => theme::WARN,
                    (MixRow::Solo, "S") => theme::GOOD,
                    (MixRow::Mute | MixRow::Solo, _) => theme::FAINT,
                    (MixRow::DelaySend | MixRow::ReverbSend, "000") => theme::FAINT,
                    _ => theme::ACCENT,
                };
                p.text(x, y, &value, color)?;
            }
        }
        if r == MixRow::Volume {
            let gain = master_gain_label(app.project.master.gain);
            let gain = if large {
                format!("{:+}", app.project.master.gain)
            } else {
                gain
            };
            let x = col_x(TRACKS);
            if selected_row && app.mix.master {
                p.text_inverted(x, y, &gain, cursor)?;
            } else {
                p.text(x, y, &gain, theme::ACCENT)?;
            }
        }
        y += ch;
    }

    y += 4;
    p.fill(0, y, WIDTH, 1, theme::LINE)?;
    y += 5;

    // Efectos y master.
    let item_cols = if large { 9 } else { 14 };
    for (i, &r) in MixRow::ALL
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.is_channel())
    {
        let selected_row = app.mix.row == i;
        if selected_row {
            p.fill(0, y, WIDTH, ch, theme::ROW_HI)?;
            p.text(x0 - 6, y, ">", cursor)?;
        }
        let name = match (large, r) {
            (true, MixRow::Delay) => "DLY",
            (true, MixRow::Reverb) => "REV",
            (true, _) => "MST",
            (false, r) => r.label(),
        };
        p.text(x0 + cw / 2, y, name, theme::TEXT)?;
        let mut x = x0 + label_cols as i32 * cw;
        for (k, (name, _)) in r.items().iter().enumerate() {
            let value = fx_value(&app.project.fx, &app.project.master, r, k);
            let mut vx = x;
            if !large {
                vx = p.text(x, y, name, theme::DIM)? + cw;
            } else if r == MixRow::Master && k == 0 {
                vx = p.text(x, y, "lim", theme::DIM)? + cw;
            }
            if selected_row && app.mix.item == k {
                p.text_inverted(vx, y, &value, cursor)?;
            } else {
                p.text(vx, y, &value, theme::ACCENT)?;
            }
            x += item_cols * cw;
        }
        // Reducción del limitador (si está activo y limitó hace poco).
        if r == MixRow::Master && app.project.master.limiter && app.meters.limiter_db < 0.0 {
            let db = format!("{:.1} dB", app.meters.limiter_db).replace('.', ",");
            let text = if large {
                db
            } else {
                format!("reducción {db}")
            };
            p.text(x + cw, y, &text, theme::WARN)?;
        }
        y += ch;
    }

    // Línea de información del cursor y, si hay sticks movidos, su efecto.
    y += 6;
    p.text(x0, y, &fit(&app.mix_info(), p.cols() - 2), theme::TEXT)?;
    y += ch;
    if let Some(text) = sticks_text(view) {
        p.text(x0, y, &fit(&text, p.cols() - 2), theme::DIM)?;
    }
    let lines = 2;
    let bottom = HEIGHT - footer_height(p, lines) - ch - 2;
    message_line(p, bottom.max(y + ch), app, view.now)?;
    footer(p, app.screen, lines, &HELP)
}

/// "stick izq: corte −40 res +10" mientras se mueve algún stick.
fn sticks_text(view: &View) -> Option<String> {
    let [(lx, ly), (rx, ry)] = view.sticks;
    let units = |v: f32| (v * 64.0).round() as i32;
    let mut parts = Vec::new();
    if lx != 0.0 || ly != 0.0 {
        parts.push(format!(
            "stick izq: corte {:+} res {:+}",
            units(lx),
            units(ly)
        ));
    }
    if rx != 0.0 || ry != 0.0 {
        parts.push(format!(
            "stick der: delay {:+} reverb {:+}",
            units(rx),
            units(ry)
        ));
    }
    (!parts.is_empty()).then(|| parts.join("   "))
}
