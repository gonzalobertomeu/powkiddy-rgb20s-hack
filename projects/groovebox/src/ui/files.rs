//! Modales de archivo (spec-ui 8, 9 y 10): editor de nombres y navegador de proyectos a
//! pantalla completa; el diálogo de confirmación, en un recuadro sobre la pantalla actual.

use std::time::{SystemTime, UNIX_EPOCH};

use super::screens::{fit_path, home_relative};
use super::{
    HEIGHT, Painter, R, View, WIDTH, fit, footer, footer_height, message_line, scroll_start, theme,
};
use crate::app::App;
use crate::modal::{Browser, Confirm, Modal, NameEditor, NamePurpose, Then};
use crate::storage::{NAME_CHARS, NAME_MAX, PROJECTS_DIR};

/// Barra de título de un modal: el título a la izquierda y `right` (recortado) a la derecha.
fn modal_title(p: &mut Painter, title: &str, right: &str) -> R<i32> {
    let h = p.ch + 8;
    p.fill(0, 0, WIDTH, h, theme::PANEL)?;
    p.fill(0, h, WIDTH, 1, theme::LINE)?;
    let end = p.text(8, 4, title, theme::ACCENT)?;
    let cols = ((WIDTH - end) / p.cw - 3).max(0) as usize;
    let right = fit_path(right, cols);
    let w = p.text_width(&right);
    p.text(WIDTH - 8 - w, 4, &right, theme::DIM)?;
    Ok(h)
}

pub fn draw_full(p: &mut Painter, app: &App, view: &View) -> R<bool> {
    match &app.modal {
        Some(Modal::Name(e)) => name_editor(p, app, view, e).map(|()| true),
        Some(Modal::Browser(b)) => browser(p, app, view, b).map(|()| true),
        _ => Ok(false),
    }
}

fn name_editor(p: &mut Painter, app: &App, view: &View, e: &NameEditor) -> R {
    let title = match e.purpose {
        NamePurpose::SaveAs => "GUARDAR COMO",
        NamePurpose::Rename => "NOMBRE DEL PROYECTO",
    };
    let top = modal_title(p, title, &app.project.name)?;
    let (cw, ch) = (p.cw, p.ch);
    let step = 16;
    let x0 = (WIDTH - NAME_MAX as i32 * step) / 2;
    let mut y = top + 3 * ch;

    // Las letras, con el hueco para agregar al final, y el cursor (recuadro y ^).
    let chars = e.chars();
    for i in 0..=chars.len().min(NAME_MAX - 1) {
        let x = x0 + i as i32 * step;
        let at_cursor = i == e.cursor;
        if at_cursor {
            let color = super::cursor_color(view);
            p.outline(x - 2, y - 2, cw + 4, ch + 4, 1, color)?;
            p.text(x, y + ch + 2, "^", color)?;
        }
        match chars.get(i) {
            Some(&c) => {
                let s = char::from(c).to_string();
                p.text(x, y, &s, theme::TEXT)?;
            }
            None => {
                p.fill(x, y + ch - 3, cw, 2, theme::FAINT)?;
            }
        }
    }
    y += 3 * ch;

    // El alfabeto, con la letra bajo el cursor resaltada.
    let astep = cw + 4;
    let ax0 = (WIDTH - NAME_CHARS.len() as i32 * astep) / 2;
    for (i, &c) in NAME_CHARS.iter().enumerate() {
        let x = ax0 + i as i32 * astep;
        let s = char::from(c).to_string();
        if e.current() == Some(c) {
            p.text_inverted(x, y, &s, theme::ACCENT)?;
        } else {
            p.text(x, y, &s, theme::DIM)?;
        }
    }
    y += 2 * ch;

    let name = e.name();
    let target = if name.is_empty() {
        "(vacío)".to_string()
    } else {
        format!("{PROJECTS_DIR}/{name}/")
    };
    let note = match e.purpose {
        NamePurpose::SaveAs => format!("se guarda una copia en {target}"),
        NamePurpose::Rename => format!("la carpeta pasa a ser {target}"),
    };
    let cols = p.cols() - 2;
    p.text(8, y, &fit(&note, cols), theme::DIM)?;
    y += ch;
    p.text(
        8,
        y,
        &fit(&format!("a-z 0-9 _ -, hasta {NAME_MAX}"), cols),
        theme::FAINT,
    )?;

    let lines = 2;
    message_line(p, HEIGHT - footer_height(p, lines) - ch - 2, app, view.now)?;
    footer(
        p,
        app.screen,
        lines,
        &[
            "izq/der mover",
            "A+arr/abj letra",
            "A+B borrar",
            "Start aceptar",
            "B cancelar",
        ],
    )
}

fn browser(p: &mut Painter, app: &App, view: &View, b: &Browser) -> R {
    let folder = app.data.as_ref().map_or(String::new(), |d| {
        format!("{}/", home_relative(&d.root().join(PROJECTS_DIR)))
    });
    let top = modal_title(p, "CARGAR PROYECTO", &folder)?;
    let (cw, ch) = (p.cw, p.ch);
    let lines = 2;
    let bottom = HEIGHT - footer_height(p, lines) - ch - 4;
    let y0 = top + 8;

    if b.entries.is_empty() {
        p.text(8 + 2 * cw, y0, "no hay proyectos guardados", theme::DIM)?;
    } else {
        p.text(8 + 2 * cw, y0, "PROYECTO", theme::FAINT)?;
        let date_x = WIDTH - 8 - 16 * cw;
        p.text(date_x, y0, "GUARDADO (UTC)", theme::FAINT)?;
        let list_y = y0 + ch + 4;
        let visible = ((bottom - list_y) / ch).max(1) as usize;
        let start = scroll_start(b.entries.len(), b.cursor, visible);
        let name_cols = ((date_x - (8 + 2 * cw)) / cw - 1) as usize;
        for (row, (i, entry)) in b
            .entries
            .iter()
            .enumerate()
            .skip(start)
            .take(visible)
            .enumerate()
        {
            let y = list_y + row as i32 * ch;
            let selected = i == b.cursor;
            if selected {
                p.fill(0, y, WIDTH, ch, theme::ROW_HI)?;
                p.text(8, y, ">", super::cursor_color(view))?;
            }
            let mut label = entry.name.clone();
            if entry.name == app.project.name {
                label.push_str("  (abierto)");
            }
            if !entry.has_main {
                label.push_str("  solo .bak");
            }
            let label = fit(&label, name_cols);
            if selected {
                p.text_inverted(8 + 2 * cw, y, &label, theme::TEXT)?;
            } else {
                let color = if entry.has_main {
                    theme::TEXT
                } else {
                    theme::WARN
                };
                p.text(8 + 2 * cw, y, &label, color)?;
            }
            let date = entry.modified.map_or("-".to_string(), format_utc);
            p.text(date_x, y, &date, theme::DIM)?;
        }
    }

    message_line(p, bottom + 2, app, view.now)?;
    footer(
        p,
        app.screen,
        lines,
        &["A abrir", "B cancelar", "B+arr/abj página"],
    )
}

/// `AAAA-MM-DD HH:MM` en UTC (sin depender de la zona horaria de la consola).
fn format_utc(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// Días desde 1970-01-01 a fecha civil (algoritmo de Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Parte `s` en líneas de hasta `max` caracteres, por palabras.
fn wrap(s: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        let needed = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if !line.is_empty() && needed > max {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines.into_iter().map(|l| fit(&l, max)).collect()
}

/// Diálogo de confirmación (spec-ui 10) sobre la pantalla actual.
pub fn confirm(p: &mut Painter, app: &App) -> R {
    let Some(Modal::Confirm(c)) = &app.modal else {
        return Ok(());
    };
    let name = &app.project.name;
    let (text, options): (Vec<String>, &[&str]) = match c {
        Confirm::Unsaved(then) => {
            let question = match then {
                Then::Quit => "¿Salir igual?".to_string(),
                Then::New => "¿Crear un proyecto nuevo igual?".to_string(),
                Then::Load(other) => format!("¿Abrir \"{other}\" igual?"),
            };
            let options: &[&str] = match then {
                Then::Quit => &["Start guardar y salir", "A salir sin guardar", "B cancelar"],
                Then::New => &[
                    "Start guardar y crear el nuevo",
                    "A crear sin guardar",
                    "B cancelar",
                ],
                Then::Load(_) => &["Start guardar y abrir", "A abrir sin guardar", "B cancelar"],
            };
            (
                vec![format!("Hay cambios sin guardar en \"{name}\"."), question],
                options,
            )
        }
        Confirm::OpenBackup { name, reason } => (
            vec![
                format!("No se puede abrir \"{name}\":"),
                reason.clone(),
                "¿Abrir la copia anterior (proyecto.ron.bak)?".to_string(),
            ],
            &["A abrir la copia", "B cancelar"],
        ),
        Confirm::Overwrite { name } => (
            vec![
                format!("Ya existe un proyecto \"{name}\"."),
                "¿Reemplazarlo? (su versión anterior queda en el .bak)".to_string(),
            ],
            &["A reemplazar", "B cancelar"],
        ),
    };

    let (cw, ch) = (p.cw, p.ch);
    let max_cols = ((WIDTH - 48) / cw) as usize;
    let mut lines: Vec<String> = text.iter().flat_map(|t| wrap(t, max_cols)).collect();
    lines.truncate(6);
    let widest = lines
        .iter()
        .map(|l| l.chars().count())
        .chain(options.iter().map(|o| o.chars().count()))
        .max()
        .unwrap_or(0) as i32;
    let w = (widest * cw + 32).min(WIDTH - 16);
    let h = (lines.len() + options.len()) as i32 * ch + ch + 24;
    let (x0, y0) = ((WIDTH - w) / 2, (HEIGHT - h) / 2);
    p.fill(x0, y0, w, h, theme::PANEL)?;
    p.outline(x0, y0, w, h, 2, theme::WARN)?;
    let mut y = y0 + 12;
    for line in &lines {
        p.text(x0 + 16, y, line, theme::TEXT)?;
        y += ch;
    }
    y += ch;
    for option in options {
        let (key, desc) = option.split_once(' ').unwrap_or((option, ""));
        let end = p.text(x0 + 16, y, key, theme::ACCENT)?;
        p.text(end + cw, y, desc, theme::TEXT)?;
        y += ch;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn utc_dates() {
        assert_eq!(format_utc(UNIX_EPOCH), "1970-01-01 00:00");
        let t = UNIX_EPOCH + Duration::from_secs(1_795_181_580);
        assert_eq!(format_utc(t), "2026-11-20 13:33");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(format_utc(leap), "2000-02-29 00:00");
    }

    #[test]
    fn wrap_by_words() {
        assert_eq!(wrap("uno dos tres", 7), ["uno dos", "tres"]);
        assert_eq!(wrap("", 7), Vec::<String>::new());
        assert_eq!(wrap("palabralarguísima", 5), ["palab"]);
    }
}
