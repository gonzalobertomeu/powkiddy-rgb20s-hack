//! Dibujo de las pantallas con primitivas de SDL2 y la fuente bitmap. Lee el estado de `App`
//! y no lo modifica.

mod files;
mod font;
mod grid;
mod mix;
mod phrase;
mod screens;
mod selection;

use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;

use rgb20s_common::audio_state::AudioState;
use rgb20s_common::battery::Battery;

use crate::app::{App, FontSize, Screen};
use crate::input::Button;
use crate::model::TrackKind;

pub use font::{BitmapFont, FontAtlas, SPLEEN_8X16, SPLEEN_12X24};

pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 480;

type R<T = ()> = Result<T, String>;

pub mod theme {
    use super::*;

    pub const BG: Color = Color::RGB(13, 14, 20);
    pub const PANEL: Color = Color::RGB(24, 26, 38);
    pub const ROW_HI: Color = Color::RGB(25, 28, 41);
    pub const LINE: Color = Color::RGB(50, 54, 76);
    pub const TEXT: Color = Color::RGB(228, 230, 238);
    pub const DIM: Color = Color::RGB(136, 141, 164);
    pub const FAINT: Color = Color::RGB(74, 78, 100);
    /// Teclas en la ayuda, nombre de la pantalla, columna del cursor.
    pub const ACCENT: Color = Color::RGB(255, 214, 102);
    /// Cursor mientras se mantiene A (valor en edición).
    pub const EDIT: Color = Color::RGB(255, 92, 120);
    pub const WARN: Color = Color::RGB(255, 128, 104);
    pub const GOOD: Color = Color::RGB(110, 220, 140);
    pub const STEP_EMPTY: Color = Color::RGB(38, 41, 58);
    pub const STEP_BEAT: Color = Color::RGB(54, 58, 82);

    pub fn track(kind: TrackKind) -> Color {
        match kind {
            TrackKind::Drum => Color::RGB(255, 140, 66),
            TrackKind::Sample => Color::RGB(64, 210, 190),
            TrackKind::Synth => Color::RGB(180, 132, 255),
        }
    }

    /// Interpolación lineal de `a` a `b`.
    pub fn mix(a: Color, b: Color, t: f32) -> Color {
        let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
        Color::RGB(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b))
    }
}

pub struct Fonts<'t> {
    pub small: FontAtlas<'t>,
    pub large: FontAtlas<'t>,
}

impl<'t> Fonts<'t> {
    pub fn new(creator: &'t TextureCreator<WindowContext>) -> R<Self> {
        Ok(Self {
            small: FontAtlas::new(BitmapFont::parse_bdf(SPLEEN_8X16)?, creator)?,
            large: FontAtlas::new(BitmapFont::parse_bdf(SPLEEN_12X24)?, creator)?,
        })
    }
}

/// Lo que la pantalla necesita del input además de `App`.
pub struct View {
    pub held: [bool; Button::ALL.len()],
    /// `(x, y)` de los sticks izquierdo y derecho, en -1..=1.
    pub sticks: [(f32, f32); 2],
    pub now: u64,
    pub system: SystemStatus,
}

/// Batería y volumen del sistema, a la derecha de la barra de título.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemStatus {
    pub battery: Option<Battery>,
    /// Estado del servicio `rgb20s-audio` (None si no está).
    pub audio: Option<AudioState>,
    /// Hubo una pulsación de volumen o un cambio de modo hace poco: se resalta.
    pub highlight: bool,
}

/// Carga desde la que la batería se dibuja como baja.
const LOW_BATTERY: u8 = 15;

impl View {
    pub fn is_held(&self, b: Button) -> bool {
        self.held[b.index()]
    }
}

pub struct Painter<'a, 't> {
    canvas: &'a mut WindowCanvas,
    fonts: &'a mut Fonts<'t>,
    large: bool,
    /// Ancho y alto de la celda de la fuente actual.
    pub cw: i32,
    pub ch: i32,
    system: SystemStatus,
}

impl Painter<'_, '_> {
    /// Columnas de texto que entran en la pantalla.
    pub fn cols(&self) -> usize {
        (WIDTH / self.cw) as usize
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) -> R {
        if w <= 0 || h <= 0 {
            return Ok(());
        }
        self.canvas.set_draw_color(color);
        self.canvas.fill_rect(Rect::new(x, y, w as u32, h as u32))
    }

    /// Borde de `t` píxeles hacia adentro del rectángulo.
    pub fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, t: i32, color: Color) -> R {
        self.fill(x, y, w, t, color)?;
        self.fill(x, y + h - t, w, t, color)?;
        self.fill(x, y, t, h, color)?;
        self.fill(x + w - t, y, t, h, color)
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) -> R {
        self.canvas.set_draw_color(color);
        self.canvas
            .draw_line(Point::new(x0, y0), Point::new(x1, y1))
    }

    pub fn text(&mut self, x: i32, y: i32, s: &str, color: Color) -> R<i32> {
        let canvas = &mut *self.canvas;
        let font = if self.large {
            &mut self.fonts.large
        } else {
            &mut self.fonts.small
        };
        font.draw(canvas, x, y, s, color)
    }

    /// Texto con la fuente chica, sin importar el tamaño elegido (etiquetas de gráficos).
    pub fn text_small(&mut self, x: i32, y: i32, s: &str, color: Color) -> R<i32> {
        self.fonts.small.draw(self.canvas, x, y, s, color)
    }

    pub fn text_width(&self, s: &str) -> i32 {
        s.chars().count() as i32 * self.cw
    }

    /// Texto con fondo: el cursor se dibuja invertido.
    pub fn text_inverted(&mut self, x: i32, y: i32, s: &str, bg: Color) -> R<i32> {
        let w = self.text_width(s);
        self.fill(x - 2, y, w + 4, self.ch, bg)?;
        self.text(x, y, s, theme::BG)
    }

    /// Escribe segmentos de colores seguidos. Devuelve la x final.
    pub fn spans(&mut self, x: i32, y: i32, spans: &[(&str, Color)]) -> R<i32> {
        let mut cx = x;
        for (s, c) in spans {
            cx = self.text(cx, y, s, *c)?;
        }
        Ok(cx)
    }
}

/// Recorta `s` a `max` caracteres, en el último espacio si lo hay (sin cortar palabras).
pub fn fit(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max + 1).collect();
    match cut.rfind(' ') {
        Some(i) if i > 0 => cut[..i].trim_end().to_string(),
        _ => cut.chars().take(max).collect(),
    }
}

/// Primera fila visible para que `cursor` quede a la vista en una lista de `total` filas.
pub fn scroll_start(total: usize, cursor: usize, visible: usize) -> usize {
    if total <= visible {
        0
    } else {
        cursor
            .saturating_sub(visible / 2)
            .min(total.saturating_sub(visible))
    }
}

pub fn cursor_color(view: &View) -> Color {
    if view.is_held(Button::A) {
        theme::EDIT
    } else {
        theme::TEXT
    }
}

/// Alto de la barra de título.
pub fn title_height(p: &Painter) -> i32 {
    p.ch + 8
}

/// Partes del indicador del sistema (batería y volumen), en el orden en que se dibujan.
fn system_parts(s: &SystemStatus) -> Vec<(SystemIcon, String)> {
    let mut parts = Vec::new();
    if let Some(b) = s.battery {
        parts.push((SystemIcon::Battery(b), format!("{}%", b.percent)));
    }
    if let Some(a) = s.audio {
        parts.push((SystemIcon::Volume, format!("{}%", a.volume_pct)));
    }
    parts
}

#[derive(Clone, Copy)]
enum SystemIcon {
    Battery(Battery),
    Volume,
}

/// Ancho de un ícono: dos celdas de la fuente (el espacio de separación incluido).
fn icon_width(p: &Painter) -> i32 {
    2 * p.cw
}

/// Columnas que ocupa el indicador del sistema (0 si no hay nada que mostrar).
fn system_cols(p: &Painter) -> usize {
    let parts = system_parts(&p.system);
    let chars: usize = parts.iter().map(|(_, t)| 2 + t.chars().count()).sum();
    chars + parts.len().saturating_sub(1)
}

/// Dibuja el indicador del sistema terminando en `right`.
fn system_status(p: &mut Painter, right: i32, y: i32) -> R {
    let s = p.system;
    let parts = system_parts(&s);
    let mut x = right - system_cols(p) as i32 * p.cw;
    for (i, (icon, text)) in parts.iter().enumerate() {
        if i > 0 {
            x += p.cw;
        }
        let w = icon_width(p) + p.text_width(text);
        let (fg, bg) = match icon {
            SystemIcon::Battery(b) if b.charging => (theme::GOOD, None),
            SystemIcon::Battery(b) if b.percent <= LOW_BATTERY => (theme::WARN, None),
            SystemIcon::Battery(_) => (theme::DIM, None),
            SystemIcon::Volume if s.highlight => (theme::BG, Some(theme::ACCENT)),
            SystemIcon::Volume => (theme::DIM, None),
        };
        if let Some(bg) = bg {
            p.fill(x - 3, y, w + 6, p.ch, bg)?;
        }
        match icon {
            SystemIcon::Battery(b) => battery_icon(p, x, y, b.percent, fg)?,
            SystemIcon::Volume => speaker_icon(p, x, y, fg)?,
        }
        p.text(x + icon_width(p), y, text, fg)?;
        x += w;
    }
    Ok(())
}

/// Pila horizontal con la carga como relleno, centrada en la celda.
fn battery_icon(p: &mut Painter, x: i32, y: i32, percent: u8, color: Color) -> R {
    let h = p.ch / 2;
    let w = icon_width(p) - p.cw / 2 - 2;
    let top = y + (p.ch - h) / 2;
    p.outline(x, top, w, h, 1, color)?;
    p.fill(x + w, top + h / 4, 2, h - 2 * (h / 4), color)?;
    let inner = w - 4;
    let level = (inner * i32::from(percent.min(100)) + 50) / 100;
    p.fill(x + 2, top + 2, level.max(1), h - 4, color)
}

/// Parlante: un cuadrado y un cono que se abre hacia la derecha.
fn speaker_icon(p: &mut Painter, x: i32, y: i32, color: Color) -> R {
    let h = p.ch / 2;
    let mid = y + p.ch / 2;
    let body = (h / 3).max(2);
    p.fill(x + 1, mid - body / 2, body, body, color)?;
    let cone = icon_width(p) - p.cw / 2 - body - 2;
    for i in 0..cone {
        let half = body / 2 + (h / 2 - body / 2) * i / cone.max(1);
        p.fill(x + 1 + body + i, mid - half, 1, 2 * half + 1, color)?;
    }
    Ok(())
}

/// Barra de título: nombre de la pantalla, contexto y, a la derecha, lo que entre de `right` y
/// el indicador del sistema (batería y volumen), que tiene su lugar reservado.
pub fn title_bar(p: &mut Painter, screen: Screen, context: &str, right: &[impl AsRef<str>]) -> R {
    let h = title_height(p);
    p.fill(0, 0, WIDTH, h, theme::PANEL)?;
    p.fill(0, h, WIDTH, 1, theme::LINE)?;
    let y = 4;
    let name = screen.title();
    let x = p.text(8, y, name, theme::ACCENT)?;
    let used = name.chars().count() + 2;
    let system = system_cols(p);
    system_status(p, WIDTH - 8, y)?;
    let cols = p.cols() - 2 - if system > 0 { system + 2 } else { 0 };
    let context = fit(context, cols.saturating_sub(used));
    p.text(x + 2 * p.cw, y, &context, theme::TEXT)?;
    let used = used + context.chars().count() + 2;
    // De `right` se dibujan las primeras partes que entren, alineadas a la derecha.
    let mut text = String::new();
    for part in right {
        let part = part.as_ref();
        let candidate = if text.is_empty() {
            part.to_string()
        } else {
            format!("{text}  {part}")
        };
        if used + candidate.chars().count() > cols {
            break;
        }
        text = candidate;
    }
    let w = p.text_width(&text);
    let end = if system > 0 {
        WIDTH - 8 - (system as i32 + 2) * p.cw
    } else {
        WIDTH - 8
    };
    p.text(end - w, y, &text, theme::DIM)?;
    Ok(())
}

/// Partes del lado derecho del título comunes a todas las pantallas (se muestran las que entren).
pub fn status_parts(app: &App) -> Vec<String> {
    // La fuente no tiene ▶: "> " = sonando, "■ " = parado.
    let state = if app.transport.is_some() { ">" } else { "■" };
    let mut parts = vec![
        format!("{state} {:.1} BPM", app.project.tempo_bpm),
        app.project_label(),
        format!("oct {}", app.octave),
    ];
    if let Some(load) = &app.audio_load {
        parts.push(format!("CPU {:.0}%", load.cpu_avg.max(load.cpu_peak)));
    }
    parts
}

/// Carga del motor para la pantalla Proyecto (entra con la fuente de 12×24).
pub fn load_text(app: &App) -> String {
    match &app.audio_load {
        Some(l) => format!(
            "{:.1}% (pico {:.1}%) {}/{} voces",
            l.cpu_avg,
            l.cpu_peak,
            l.voices_max,
            crate::engine::pool::POOL_VOICES + crate::engine::pool::FADE_SLOTS,
        ),
        None => "sin audio".to_string(),
    }
}

const MINIMAP_BOX: (i32, i32) = (18, 7);
const MINIMAP_GAP: i32 = 2;
const MINIMAP_W: i32 = 5 * MINIMAP_BOX.0 + 4 * MINIMAP_GAP;
const MINIMAP_H: i32 = 3 * MINIMAP_BOX.1 + 2 * MINIMAP_GAP;

/// Posición en el mapa (columna, fila) de cada pantalla.
fn map_position(s: Screen) -> (i32, i32) {
    match s {
        Screen::Song => (0, 1),
        Screen::Grid => (1, 1),
        Screen::Chain => (2, 1),
        Screen::Phrase => (3, 1),
        Screen::Instrument => (4, 1),
        Screen::Project => (2, 0),
        Screen::Mix => (2, 2),
    }
}

const ALL_SCREENS: [Screen; 7] = [
    Screen::Song,
    Screen::Grid,
    Screen::Chain,
    Screen::Phrase,
    Screen::Instrument,
    Screen::Project,
    Screen::Mix,
];

/// Minimapa chico: un bloque por pantalla, la actual resaltada.
fn minimap(p: &mut Painter, x: i32, y: i32, current: Screen) -> R {
    let (bw, bh) = MINIMAP_BOX;
    for s in ALL_SCREENS {
        let (c, r) = map_position(s);
        let color = if s == current {
            theme::ACCENT
        } else {
            theme::LINE
        };
        p.fill(
            x + c * (bw + MINIMAP_GAP),
            y + r * (bh + MINIMAP_GAP),
            bw,
            bh,
            color,
        )?;
    }
    Ok(())
}

/// Minimapa grande mientras se mantiene Select (spec-ui, mapa global de controles).
fn minimap_overlay(p: &mut Painter, current: Screen) -> R {
    let hint = "Select + cruz: cambiar de pantalla";
    let (bw, bh) = (p.cw * 3 + 16, p.ch + 10);
    let gap = 8;
    let w = (5 * bw + 4 * gap + 32).max(p.text_width(hint) + 32);
    let h = 3 * bh + 2 * gap + 32 + p.ch + 8;
    let (x0, y0) = ((WIDTH - w) / 2, (HEIGHT - h) / 2);
    p.fill(x0, y0, w, h, theme::PANEL)?;
    p.outline(x0, y0, w, h, 2, theme::ACCENT)?;
    let boxes_x0 = x0 + (w - (5 * bw + 4 * gap)) / 2;
    for s in ALL_SCREENS {
        let (c, r) = map_position(s);
        let (x, y) = (boxes_x0 + c * (bw + gap), y0 + 16 + r * (bh + gap));
        let label = s.short();
        if s == current {
            p.fill(x, y, bw, bh, theme::ACCENT)?;
            p.text(x + 8, y + 5, label, theme::BG)?;
        } else {
            p.outline(x, y, bw, bh, 1, theme::LINE)?;
            p.text(x + 8, y + 5, label, theme::TEXT)?;
        }
    }
    let hw = p.text_width(hint);
    p.text(x0 + (w - hw) / 2, y0 + h - p.ch - 10, hint, theme::DIM)?;
    Ok(())
}

/// Alto del pie con `lines` líneas de texto.
pub fn footer_height(p: &Painter, lines: i32) -> i32 {
    (lines * p.ch + 8).max(MINIMAP_H + 6)
}

/// Pie: ayuda (teclas en color) en `lines` líneas y el minimapa a la derecha.
/// Cada ítem de `help` es "tecla descripción"; la primera palabra es la tecla.
pub fn footer(p: &mut Painter, screen: Screen, lines: i32, help: &[&str]) -> R {
    let h = footer_height(p, lines);
    let y0 = HEIGHT - h;
    p.fill(0, y0, WIDTH, h, theme::PANEL)?;
    p.fill(0, y0, WIDTH, 1, theme::LINE)?;
    minimap(p, WIDTH - 8 - MINIMAP_W, y0 + (h - MINIMAP_H) / 2, screen)?;

    let max_x = WIDTH - 8 - MINIMAP_W - 12;
    let (mut x, mut line) = (8, 0);
    let y_of = |p: &Painter, line: i32| y0 + 4 + line * p.ch;
    for item in help {
        let w = p.text_width(item);
        if x > 8 && x + w > max_x {
            line += 1;
            x = 8;
        }
        if line >= lines {
            break;
        }
        let (key, desc) = item.split_once(' ').unwrap_or((item, ""));
        let y = y_of(p, line);
        let after_key = p.text(x, y, key, theme::ACCENT)?;
        let end = p.text(after_key + p.cw, y, desc, theme::DIM)?;
        x = end + 2 * p.cw;
    }
    Ok(())
}

/// Línea de mensaje transitorio (avisos en otro color).
pub fn message_line(p: &mut Painter, y: i32, app: &App, now: u64) -> R {
    if let Some(m) = app.message_at(now) {
        let color = if m.warn { theme::WARN } else { theme::GOOD };
        let mark = if m.warn { "! " } else { "· " };
        let text = fit(&format!("{mark}{}", m.text), p.cols() - 2);
        p.text(8, y, &text, color)?;
    }
    Ok(())
}

/// Dibuja la pantalla en el buffer; quien llama hace `present` (antes se puede capturar).
pub fn draw(canvas: &mut WindowCanvas, fonts: &mut Fonts, app: &App, view: &View) -> R {
    canvas.set_draw_color(theme::BG);
    canvas.clear();
    let large = app.font == FontSize::Large;
    let (cw, ch) = if large {
        fonts.large.cell()
    } else {
        fonts.small.cell()
    };
    let mut p = Painter {
        canvas,
        fonts,
        large,
        cw,
        ch,
        system: view.system,
    };
    // El editor de nombres y el navegador ocupan toda la pantalla; el diálogo va encima.
    if files::draw_full(&mut p, app, view)? {
        return Ok(());
    }
    match app.screen {
        Screen::Grid => grid::draw(&mut p, app, view)?,
        Screen::Phrase => phrase::draw(&mut p, app, view)?,
        Screen::Song => screens::song(&mut p, app, view)?,
        Screen::Chain => screens::chain(&mut p, app, view)?,
        Screen::Project => screens::project(&mut p, app, view)?,
        Screen::Instrument => screens::instrument(&mut p, app, view)?,
        Screen::Mix => mix::draw(&mut p, app, view)?,
    }
    if app.modal.is_some() {
        files::confirm(&mut p, app)?;
    } else if view.is_held(Button::Select) {
        minimap_overlay(&mut p, app.screen)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_keeps_cursor_visible() {
        assert_eq!(scroll_start(10, 9, 20), 0);
        assert_eq!(scroll_start(256, 0, 16), 0);
        assert_eq!(scroll_start(256, 100, 16), 92);
        assert_eq!(scroll_start(256, 255, 16), 240);
    }

    #[test]
    fn fit_cuts_at_word_boundary() {
        assert_eq!(fit("hola mundo", 20), "hola mundo");
        assert_eq!(fit("hola mundo", 7), "hola");
        assert_eq!(fit("hola mundo", 4), "hola");
        assert_eq!(fit("holamundo", 4), "hola");
        assert_eq!(fit("áé íó", 3), "áé");
    }

    #[test]
    fn mix_endpoints() {
        let a = Color::RGB(0, 100, 200);
        let b = Color::RGB(200, 0, 100);
        assert_eq!(theme::mix(a, b, 0.0), a);
        assert_eq!(theme::mix(a, b, 1.0), b);
        assert_eq!(theme::mix(a, b, 0.5), Color::RGB(100, 50, 150));
    }
}
