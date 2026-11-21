//! Lo que el modo selección agrega a las pantallas (H10): el color del resaltado y el pie con
//! el indicador SELECCIÓN y sus controles.

use sdl2::pixels::Color;

use super::{HEIGHT, Painter, R, footer, footer_height, theme};
use crate::app::{App, Screen};

/// Fondo de lo seleccionado (el texto encima sigue legible).
pub const SELECT: Color = Color::RGB(48, 92, 170);

/// La celda `(x, y)` de la pantalla está seleccionada.
pub fn selected(app: &App, x: usize, y: usize) -> bool {
    app.selection_area().is_some_and(|a| a.contains(x, y))
}

/// Pie de la pantalla: con una selección abierta, "SELECCIÓN" (invertido), su tamaño y los
/// controles del modo; si no, la ayuda normal.
pub fn footer_or_selection(p: &mut Painter, app: &App, lines: i32, help: &[&str]) -> R {
    let Some(label) = app.selection_label() else {
        return footer(p, app.screen, lines, help);
    };
    let tag = "SELECCIÓN";
    let head = format!("{tag} {label}");
    let widen = app
        .selection_next_level()
        .map(|next| format!("Y ampliar a {next}"));
    // Un instrumento no se extiende ni se corta: solo copiar o cancelar.
    let whole = app.screen == Screen::Instrument;
    // Primero lo que cierra la selección: con la fuente grande no entra todo.
    let mut items = vec![head.as_str(), "A copiar"];
    if !whole {
        items.push("X cortar");
    }
    items.push("B cancelar");
    if let Some(w) = &widen {
        items.push(w);
    }
    if !whole {
        items.push("cruz extender");
    }
    footer(p, app.screen, lines, &items)?;
    let y = HEIGHT - footer_height(p, lines) + 4;
    p.text_inverted(8, y, tag, theme::ACCENT)?;
    Ok(())
}
