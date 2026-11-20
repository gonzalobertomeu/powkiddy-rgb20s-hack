//! Fuentes bitmap embebidas en el binario (Spleen 8×16 y 12×24, BSD-2-Clause; ver
//! `fonts/LICENSE-spleen`). Se parsea el BDF al arrancar y se arma un atlas en una textura.

use std::collections::HashMap;

use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;

pub const SPLEEN_8X16: &str = include_str!("../fonts/spleen-8x16.bdf");
pub const SPLEEN_12X24: &str = include_str!("../fonts/spleen-12x24.bdf");

/// Fuente de ancho fijo: un mapa de bits de `width × height` (un byte por píxel, 0 o 1) por glifo.
pub struct BitmapFont {
    pub width: u32,
    pub height: u32,
    pixels: Vec<u8>,
    index: HashMap<char, usize>,
    fallback: usize,
}

fn numbers<const N: usize>(line: &str) -> Option<[i32; N]> {
    let mut out = [0; N];
    let mut parts = line.split_whitespace().skip(1);
    for v in &mut out {
        *v = parts.next()?.parse().ok()?;
    }
    Some(out)
}

impl BitmapFont {
    /// Parser mínimo de BDF 2.1: alcanza para fuentes de ancho fijo como Spleen.
    pub fn parse_bdf(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let mut bbox = None;
        for line in lines.by_ref() {
            if line.starts_with("FONTBOUNDINGBOX") {
                bbox = numbers::<4>(line);
                break;
            }
        }
        let [width, height, fx, fy] = bbox.ok_or("BDF sin FONTBOUNDINGBOX")?;
        if width <= 0 || height <= 0 {
            return Err("BDF con tamaño inválido".into());
        }
        let (w, h) = (width as usize, height as usize);
        // Fila (desde arriba) de la línea de base dentro de la celda.
        let baseline = height + fy;

        let mut font = Self {
            width: width as u32,
            height: height as u32,
            pixels: Vec::new(),
            index: HashMap::new(),
            fallback: 0,
        };
        let mut encoding: Option<u32> = None;
        let mut glyph_box = [width, height, fx, fy];
        while let Some(line) = lines.next() {
            if line.starts_with("ENCODING") {
                encoding = numbers::<1>(line).and_then(|[e]| u32::try_from(e).ok());
            } else if line.starts_with("BBX") {
                glyph_box = numbers::<4>(line).ok_or("BBX inválido")?;
            } else if line == "BITMAP" {
                let [gw, gh, gx, gy] = glyph_box;
                let mut cell = vec![0u8; w * h];
                let top = baseline - (gh + gy);
                for row in 0..gh {
                    let hex = lines.next().ok_or("BITMAP incompleto")?;
                    let bits = u64::from_str_radix(hex.trim(), 16)
                        .map_err(|e| format!("BITMAP inválido: {e}"))?;
                    let total_bits = hex.trim().len() as i32 * 4;
                    for col in 0..gw {
                        let on = bits >> (total_bits - 1 - col) & 1 == 1;
                        let (x, y) = (col + gx - fx, top + row);
                        if on && (0..width).contains(&x) && (0..height).contains(&y) {
                            cell[y as usize * w + x as usize] = 1;
                        }
                    }
                }
                if let Some(c) = encoding.and_then(char::from_u32) {
                    font.index.insert(c, font.pixels.len() / (w * h));
                    font.pixels.extend_from_slice(&cell);
                }
                encoding = None;
                glyph_box = [width, height, fx, fy];
            }
        }
        font.fallback = *font.index.get(&'?').ok_or("la fuente no tiene '?'")?;
        Ok(font)
    }

    pub fn glyph_count(&self) -> usize {
        self.pixels.len() / (self.width * self.height) as usize
    }

    pub fn glyph_index(&self, c: char) -> usize {
        self.index.get(&c).copied().unwrap_or(self.fallback)
    }

    pub fn glyph(&self, index: usize) -> &[u8] {
        let n = (self.width * self.height) as usize;
        &self.pixels[index * n..(index + 1) * n]
    }
}

/// Glifos por fila del atlas: mantiene la textura chica en ancho (límite de algunas GPU).
const ATLAS_COLUMNS: u32 = 32;

pub struct FontAtlas<'t> {
    pub font: BitmapFont,
    texture: Texture<'t>,
}

impl<'t> FontAtlas<'t> {
    pub fn new(
        font: BitmapFont,
        creator: &'t TextureCreator<WindowContext>,
    ) -> Result<Self, String> {
        let (gw, gh) = (font.width, font.height);
        let rows = (font.glyph_count() as u32).div_ceil(ATLAS_COLUMNS);
        let (tw, th) = (ATLAS_COLUMNS * gw, rows * gh);
        let mut rgba = vec![0u8; (tw * th * 4) as usize];
        for i in 0..font.glyph_count() {
            let (ox, oy) = (i as u32 % ATLAS_COLUMNS * gw, i as u32 / ATLAS_COLUMNS * gh);
            for (p, &on) in font.glyph(i).iter().enumerate() {
                let (x, y) = (ox + p as u32 % gw, oy + p as u32 / gw);
                let o = ((y * tw + x) * 4) as usize;
                // Blanco con alfa: el color se aplica con color mod al dibujar.
                rgba[o..o + 4].copy_from_slice(&[255, 255, 255, if on == 1 { 255 } else { 0 }]);
            }
        }
        let mut texture = creator
            .create_texture_static(PixelFormatEnum::RGBA32, tw, th)
            .map_err(|e| e.to_string())?;
        texture
            .update(None, &rgba, (tw * 4) as usize)
            .map_err(|e| e.to_string())?;
        texture.set_blend_mode(BlendMode::BLEND);
        Ok(Self { font, texture })
    }

    pub fn cell(&self) -> (i32, i32) {
        (self.font.width as i32, self.font.height as i32)
    }

    /// Dibuja `text` desde `(x, y)` (esquina superior izquierda). Devuelve la x final.
    pub fn draw(
        &mut self,
        canvas: &mut WindowCanvas,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
    ) -> Result<i32, String> {
        let (w, h) = (self.font.width, self.font.height);
        self.texture.set_color_mod(color.r, color.g, color.b);
        let mut cx = x;
        for c in text.chars() {
            if c != ' ' {
                let i = self.font.glyph_index(c) as u32;
                let src = Rect::new(
                    (i % ATLAS_COLUMNS * w) as i32,
                    (i / ATLAS_COLUMNS * h) as i32,
                    w,
                    h,
                );
                canvas.copy(&self.texture, src, Rect::new(cx, y, w, h))?;
            }
            cx += w as i32;
        }
        Ok(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(font: &BitmapFont, c: char) -> Vec<String> {
        let g = font.glyph(font.glyph_index(c));
        g.chunks(font.width as usize)
            .map(|row| {
                row.iter()
                    .map(|&p| if p == 1 { '#' } else { '.' })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn both_sizes_parse_with_printable_ascii_and_spanish() {
        for (text, w, h) in [(SPLEEN_8X16, 8, 16), (SPLEEN_12X24, 12, 24)] {
            let font = BitmapFont::parse_bdf(text).unwrap();
            assert_eq!((font.width, font.height), (w, h));
            for c in (0x20u8..0x7f)
                .map(char::from)
                .chain("áéíóúñÁÉÍÓÚÑ¿¡·".chars())
            {
                assert!(font.index.contains_key(&c), "falta {c:?} en {w}x{h}");
            }
        }
    }

    #[test]
    fn glyph_bitmap_is_placed_in_cell() {
        let font = BitmapFont::parse_bdf(SPLEEN_8X16).unwrap();
        let space = font.glyph(font.glyph_index(' '));
        assert!(space.iter().all(|&p| p == 0));
        // En Spleen 8×16, "|" es una barra de 2 píxeles en las filas 1 a 12.
        let bar = render(&font, '|');
        assert_eq!(bar[0], "........");
        assert!(bar[1..13].iter().all(|r| r == "...##..."));
        assert_eq!(bar[13], "........");
        let at = render(&font, '@');
        assert!(at.iter().any(|r| r.contains('#')));
    }

    #[test]
    fn unknown_char_falls_back_to_question_mark() {
        let font = BitmapFont::parse_bdf(SPLEEN_8X16).unwrap();
        assert_eq!(font.glyph_index('\u{1F3B5}'), font.glyph_index('?'));
    }

    #[test]
    fn tiny_bdf_with_offset_glyph() {
        let bdf = "STARTFONT 2.1\nFONTBOUNDINGBOX 4 4 0 -1\nCHARS 2\n\
                   STARTCHAR q\nENCODING 63\nBBX 4 4 0 -1\nBITMAP\nF0\n00\n00\n00\nENDCHAR\n\
                   STARTCHAR dot\nENCODING 46\nBBX 1 1 1 0\nBITMAP\n80\nENDCHAR\nENDFONT\n";
        let font = BitmapFont::parse_bdf(bdf).unwrap();
        assert_eq!(render(&font, '?'), ["####", "....", "....", "...."]);
        // Glifo de 1×1 a 1 píxel de la izquierda, apoyado en la línea de base (fila 2).
        assert_eq!(render(&font, '.'), ["....", "....", ".#..", "...."]);
    }
}
