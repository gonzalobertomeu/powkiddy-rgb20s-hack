//! Dibujo de la pantalla con primitivas de SDL2 y la fuente bitmap. Colores del groovebox.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;

use crate::apps::{AppEntry, Entry};
use crate::battery::Battery;
use crate::font::{BitmapFont, FontAtlas, SPLEEN_8X16, SPLEEN_12X24};

pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 480;

type R<T = ()> = Result<T, String>;

mod theme {
    use sdl2::pixels::Color;

    pub const BG: Color = Color::RGB(13, 14, 20);
    pub const PANEL: Color = Color::RGB(24, 26, 38);
    pub const ROW_SEL: Color = Color::RGB(36, 40, 58);
    pub const LINE: Color = Color::RGB(50, 54, 76);
    pub const TEXT: Color = Color::RGB(228, 230, 238);
    pub const DIM: Color = Color::RGB(136, 141, 164);
    pub const FAINT: Color = Color::RGB(74, 78, 100);
    pub const ACCENT: Color = Color::RGB(255, 214, 102);
    pub const WARN: Color = Color::RGB(255, 128, 104);
    pub const GOOD: Color = Color::RGB(110, 220, 140);
}

const TITLE_H: i32 = 44;
const LIST_TOP: i32 = TITLE_H + 16;
const ROW_H: i32 = 36;
const FOOTER_TOP: i32 = HEIGHT - 32;
const MESSAGE_TOP: i32 = FOOTER_TOP - 28;
const VISIBLE_ROWS: usize = ((MESSAGE_TOP - 8 - LIST_TOP) / ROW_H) as usize;
const MARGIN: i32 = 12;
const LOW_BATTERY: u8 = 15;
const _: () = assert!(VISIBLE_ROWS >= 8 && LIST_TOP + VISIBLE_ROWS as i32 * ROW_H <= MESSAGE_TOP);

pub struct Fonts<'t> {
    small: FontAtlas<'t>,
    large: FontAtlas<'t>,
}

impl<'t> Fonts<'t> {
    pub fn new(creator: &'t TextureCreator<WindowContext>) -> R<Self> {
        Ok(Self {
            small: FontAtlas::new(BitmapFont::parse_bdf(SPLEEN_8X16)?, creator)?,
            large: FontAtlas::new(BitmapFont::parse_bdf(SPLEEN_12X24)?, creator)?,
        })
    }
}

/// Lo que se dibuja; el estado vive en `main.rs`.
pub struct Screen<'a> {
    pub apps: &'a [AppEntry],
    pub entries: &'a [Entry],
    /// Por entrada: si el comando existe (las del sistema, siempre).
    pub available: &'a [bool],
    pub selected: usize,
    pub battery: Option<Battery>,
    /// Aviso abajo de la lista (error de `apps.toml`, app que no existe).
    pub message: Option<&'a str>,
}

fn fill(canvas: &mut WindowCanvas, x: i32, y: i32, w: i32, h: i32, color: Color) -> R {
    canvas.set_draw_color(color);
    canvas.fill_rect(Rect::new(x, y, w.max(0) as u32, h.max(0) as u32))
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Primera fila visible para que `cursor` quede a la vista.
fn scroll_start(total: usize, cursor: usize, visible: usize) -> usize {
    if total <= visible {
        0
    } else {
        cursor.saturating_sub(visible / 2).min(total - visible)
    }
}

pub fn draw(canvas: &mut WindowCanvas, fonts: &mut Fonts, s: &Screen) -> R {
    canvas.set_draw_color(theme::BG);
    canvas.clear();
    let (lw, lh) = fonts.large.cell();
    let (sw, sh) = fonts.small.cell();

    // Título y batería.
    fill(canvas, 0, 0, WIDTH, TITLE_H, theme::PANEL)?;
    fill(canvas, 0, TITLE_H, WIDTH, 1, theme::LINE)?;
    let ty = (TITLE_H - lh) / 2;
    fonts.large.draw(canvas, 16, ty, "RGB20S", theme::ACCENT)?;
    if let Some(b) = s.battery {
        let label = format!("BAT {}", b.label());
        let color = match b {
            Battery { charging: true, .. } => theme::GOOD,
            Battery { percent, .. } if percent <= LOW_BATTERY => theme::WARN,
            _ => theme::DIM,
        };
        let x = WIDTH - 16 - label.chars().count() as i32 * lw;
        fonts.large.draw(canvas, x, ty, &label, color)?;
    }

    // Lista.
    let first = scroll_start(s.entries.len(), s.selected, VISIBLE_ROWS);
    let name_x = MARGIN + 20;
    let max_chars = ((WIDTH - name_x - MARGIN) / lw) as usize;
    for (row, i) in (first..s.entries.len().min(first + VISIBLE_ROWS)).enumerate() {
        let entry = s.entries[i];
        let y = LIST_TOP + row as i32 * ROW_H;
        let selected = i == s.selected;
        // Separador entre las apps y las entradas del sistema.
        if entry == Entry::Reboot && i > 0 {
            fill(canvas, MARGIN, y - 3, WIDTH - 2 * MARGIN, 1, theme::LINE)?;
        }
        if selected {
            fill(
                canvas,
                MARGIN,
                y + 1,
                WIDTH - 2 * MARGIN,
                ROW_H - 4,
                theme::ROW_SEL,
            )?;
            fill(canvas, MARGIN, y + 1, 4, ROW_H - 4, theme::ACCENT)?;
        }
        let available = s.available.get(i).copied().unwrap_or(true);
        let color = match (available, selected) {
            (false, _) => theme::FAINT,
            (true, true) => theme::TEXT,
            (true, false) => theme::DIM,
        };
        let text_y = y + 1 + (ROW_H - 4 - lh) / 2;
        let name = truncate(entry.label(s.apps), max_chars);
        let end = fonts.large.draw(canvas, name_x, text_y, &name, color)?;
        if !available {
            let note = "no encontrada";
            let x = (WIDTH - MARGIN - 8 - note.len() as i32 * sw).max(end + sw);
            fonts
                .small
                .draw(canvas, x, y + 1 + (ROW_H - 4 - sh) / 2, note, theme::FAINT)?;
        }
    }
    // Hay filas fuera de la vista.
    let arrow_x = WIDTH - MARGIN - sw;
    if first > 0 {
        fonts
            .small
            .draw(canvas, arrow_x, LIST_TOP - sh, "↑", theme::DIM)?;
    }
    if first + VISIBLE_ROWS < s.entries.len() {
        let y = LIST_TOP + VISIBLE_ROWS as i32 * ROW_H - 4;
        fonts.small.draw(canvas, arrow_x, y, "↓", theme::DIM)?;
    }

    if let Some(msg) = s.message {
        let max = ((WIDTH - 2 * 16) / sw) as usize;
        fonts.small.draw(
            canvas,
            16,
            MESSAGE_TOP + 4,
            &truncate(msg, max),
            theme::WARN,
        )?;
    }

    // Ayuda.
    fill(canvas, 0, FOOTER_TOP, WIDTH, 1, theme::LINE)?;
    let fy = FOOTER_TOP + (HEIGHT - FOOTER_TOP - sh) / 2;
    let mut x = 16;
    for (text, color) in [
        ("A", theme::ACCENT),
        (" abrir  ·  ", theme::DIM),
        ("↑↓", theme::ACCENT),
        (" elegir", theme::DIM),
    ] {
        x = fonts.small.draw(canvas, x, fy, text, color)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_keeps_cursor_visible() {
        assert_eq!(scroll_start(5, 4, 9), 0);
        for cursor in 0..20 {
            let first = scroll_start(20, cursor, 9);
            assert!((first..first + 9).contains(&cursor));
            assert!(first + 9 <= 20);
        }
    }

    #[test]
    fn help_glyphs_exist() {
        let font = BitmapFont::parse_bdf(SPLEEN_8X16).unwrap();
        let q = font.glyph_index('?');
        for c in "↑↓·".chars() {
            assert_ne!(font.glyph_index(c), q, "falta {c:?}");
        }
    }
}
