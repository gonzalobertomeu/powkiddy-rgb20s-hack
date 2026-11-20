#!/usr/bin/env python3
"""Genera el splash de arranque de la RGB20S ("KingaGroove") a partir de fuentes BDF.

Solo usa la biblioteca estándar (corre en la Mac y en el contenedor rgb20s-image-builder).
Salidas (en --outdir, por defecto ./generated junto a este script):

  splash.png                   referencia de 640x480 (para mirar y para documentación)
  splash.ppm                   P6 de 640x480: lo dibuja rgb20s-splash en userspace (DRM)
  logo_rgb20s_clut224.ppm      P3 de 640x464 (<= 224 colores): logo del kernel (CONFIG_LOGO_LINUX_CLUT224_FILE)

El logo del kernel es la franja superior de 464 px del mismo dibujo: fbcon lo pone arriba a la
izquierda y deja la última fila de texto (16 px) para mensajes. Por eso el contenido tiene que
quedar por encima de LOGO_HEIGHT.

Para cambiar el arte, editar CONFIG (textos, colores, fuentes, escalas, patrón de pasos) o pasar
--title-a/--title-b/--subtitle, y volver a correr el script (o `just img-kernel`, que lo corre).
Los archivos solo se reescriben si cambia su contenido, así el kernel no se recompila de gusto.
"""
import argparse
import os
import struct
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
FONTS = os.path.normpath(os.path.join(HERE, "..", "..", "launcher", "fonts"))

# Paleta del launcher (projects/launcher/src/ui.rs, módulo de colores)
BG = (13, 14, 20)
PANEL = (24, 26, 38)
ROW_SEL = (36, 40, 58)
LINE = (50, 54, 76)
TEXT = (228, 230, 238)
DIM = (136, 141, 164)
ACCENT = (255, 214, 102)

CONFIG = {
    "width": 640,
    "height": 480,
    # Alto del logo del kernel: múltiplo de 16 (alto de la fuente 8x16 de fbcon) y menor que la
    # pantalla, para que fbcon no lo descarte ("boot-logo bigger than screen") y quede una fila de texto.
    "logo_height": 464,
    "bg": BG,
    # Título en dos colores: "Kinga" claro + "Groove" ámbar
    "title_a": "Kinga",
    "title_b": "Groove",
    "title_a_color": TEXT,
    "title_b_color": ACCENT,
    "title_font": os.path.join(FONTS, "spleen-12x24.bdf"),
    "title_scale": 3,
    # Fila de 16 pasos (guiño al secuenciador): x = paso activo
    "steps": "x...x...x..xx...",
    "step_size": 18,
    "step_gap": 6,
    "step_on": ACCENT,
    "step_off": ROW_SEL,
    "step_beat": LINE,  # pasos 1, 5, 9 y 13 apagados: un poco más claros
    "subtitle": "groovebox · RGB20S",
    "subtitle_color": DIM,
    "subtitle_font": os.path.join(FONTS, "spleen-8x16.bdf"),
    "subtitle_scale": 1,
    # Separaciones verticales entre bloques (px)
    "gap_title_steps": 30,
    "gap_steps_subtitle": 30,
}


class BdfFont:
    """Fuente BDF monoespaciada mínima: glifos por código Unicode (ENCODING)."""

    def __init__(self, path):
        self.glyphs = {}
        self.ascent = 0
        self.height = 0
        fbb = None
        with open(path, encoding="latin-1") as f:
            lines = iter(f.read().splitlines())
        for line in lines:
            if line.startswith("FONTBOUNDINGBOX "):
                fbb = [int(v) for v in line.split()[1:]]
            elif line.startswith("FONT_ASCENT "):
                self.ascent = int(line.split()[1])
            elif line.startswith("STARTCHAR"):
                enc = dwidth = bbx = None
                rows = []
                for line in lines:
                    if line.startswith("ENCODING "):
                        enc = int(line.split()[1])
                    elif line.startswith("DWIDTH "):
                        dwidth = int(line.split()[1])
                    elif line.startswith("BBX "):
                        bbx = [int(v) for v in line.split()[1:]]
                    elif line == "BITMAP":
                        for line in lines:
                            if line == "ENDCHAR":
                                break
                            rows.append(int(line, 16))
                        break
                if enc is not None and enc >= 0 and bbx:
                    self.glyphs[enc] = (dwidth or bbx[0], bbx, rows)
        if fbb is None:
            raise ValueError(f"{path}: falta FONTBOUNDINGBOX")
        self.width, self.height = fbb[0], fbb[1]
        if not self.ascent:
            self.ascent = fbb[1] + fbb[3]

    def text_width(self, text, scale):
        return sum(self.glyph(ch)[0] for ch in text) * scale

    def glyph(self, ch):
        g = self.glyphs.get(ord(ch)) or self.glyphs.get(ord("?"))
        if g is None:
            raise ValueError(f"la fuente no tiene {ch!r} ni '?'")
        return g


class Canvas:
    def __init__(self, w, h, bg):
        self.w, self.h = w, h
        self.px = bytearray(bytes(bg) * (w * h))

    def fill(self, x, y, w, h, color):
        x0, y0 = max(0, x), max(0, y)
        x1, y1 = min(self.w, x + w), min(self.h, y + h)
        if x0 >= x1 or y0 >= y1:
            return
        row = bytes(color) * (x1 - x0)
        for yy in range(y0, y1):
            o = (yy * self.w + x0) * 3
            self.px[o:o + len(row)] = row

    def text(self, font, text, x, y, scale, color):
        """Dibuja `text` con la esquina superior izquierda de la celda en (x, y)."""
        for ch in text:
            dwidth, (bw, bh, bx, by), rows = font.glyph(ch)
            # Fila superior del bitmap respecto del tope de la celda
            top = font.ascent - (bh + by)
            nbits = ((bw + 7) // 8) * 8
            for r, bits in enumerate(rows):
                for c in range(bw):
                    if bits & (1 << (nbits - 1 - c)):
                        self.fill(x + (bx + c) * scale, y + (top + r) * scale, scale, scale, color)
            x += dwidth * scale

    def crop_rows(self, h):
        return bytes(self.px[: self.w * h * 3])


def draw(cfg):
    w, h = cfg["width"], cfg["height"]
    cv = Canvas(w, h, cfg["bg"])
    tfont = BdfFont(cfg["title_font"])
    sfont = BdfFont(cfg["subtitle_font"])
    ts, ss = cfg["title_scale"], cfg["subtitle_scale"]

    title = cfg["title_a"] + cfg["title_b"]
    title_w = tfont.text_width(title, ts)
    title_h = tfont.height * ts
    steps = cfg["steps"]
    n = len(steps)
    step_w = n * cfg["step_size"] + (n - 1) * cfg["step_gap"] if n else 0
    sub_w = sfont.text_width(cfg["subtitle"], ss) if cfg["subtitle"] else 0
    sub_h = sfont.height * ss if cfg["subtitle"] else 0

    block_h = title_h
    if n:
        block_h += cfg["gap_title_steps"] + cfg["step_size"]
    if sub_h:
        block_h += cfg["gap_steps_subtitle"] + sub_h
    y = (h - block_h) // 2
    if y + block_h > cfg["logo_height"]:
        raise ValueError("el contenido no entra en la franja del logo del kernel (logo_height)")

    x = (w - title_w) // 2
    cv.text(tfont, cfg["title_a"], x, y, ts, cfg["title_a_color"])
    cv.text(tfont, cfg["title_b"], x + tfont.text_width(cfg["title_a"], ts), y, ts, cfg["title_b_color"])
    y += title_h

    if n:
        y += cfg["gap_title_steps"]
        x = (w - step_w) // 2
        for i, s in enumerate(steps):
            if s == "x":
                color = cfg["step_on"]
            elif i % 4 == 0:
                color = cfg["step_beat"]
            else:
                color = cfg["step_off"]
            cv.fill(x + i * (cfg["step_size"] + cfg["step_gap"]), y, cfg["step_size"], cfg["step_size"], color)
        y += cfg["step_size"]

    if sub_h:
        y += cfg["gap_steps_subtitle"]
        cv.text(sfont, cfg["subtitle"], (w - sub_w) // 2, y, ss, cfg["subtitle_color"])
    return cv


def png_bytes(w, h, rgb):
    raw = b"".join(b"\x00" + rgb[y * w * 3:(y + 1) * w * 3] for y in range(h))

    def chunk(kind, data):
        c = kind + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def ppm_p6(w, h, rgb):
    return b"P6\n# rgb20s splash (make-splash.py)\n%d %d\n255\n" % (w, h) + rgb


def ppm_p3(w, h, rgb):
    """PPM de texto (pnmtologo del kernel no acepta P6)."""
    out = ["P3", "# rgb20s: logo del kernel (make-splash.py)", f"{w} {h}", "255"]
    per_line = 5  # píxeles por línea: < 70 caracteres
    vals = [f"{rgb[i]} {rgb[i + 1]} {rgb[i + 2]}" for i in range(0, len(rgb), 3)]
    for i in range(0, len(vals), per_line):
        out.append("  ".join(vals[i:i + per_line]))
    return ("\n".join(out) + "\n").encode()


def write_if_changed(path, data):
    try:
        with open(path, "rb") as f:
            if f.read() == data:
                return False
    except FileNotFoundError:
        pass
    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(data)
    os.replace(tmp, path)
    return True


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--outdir", default=os.path.join(HERE, "generated"))
    ap.add_argument("--title-a", help=f"primera parte del título (defecto {CONFIG['title_a']!r})")
    ap.add_argument("--title-b", help=f"segunda parte, en color de acento (defecto {CONFIG['title_b']!r})")
    ap.add_argument("--subtitle", help="texto chico de abajo ('' = sin subtítulo)")
    ap.add_argument("--only", choices=["png", "ppm", "kernel"], action="append",
                    help="generar solo esa salida (repetible)")
    a = ap.parse_args()
    cfg = dict(CONFIG)
    for k in ("title_a", "title_b", "subtitle"):
        v = getattr(a, k)
        if v is not None:
            cfg[k] = v
    only = set(a.only or ["png", "ppm", "kernel"])

    cv = draw(cfg)
    w, h, lh = cfg["width"], cfg["height"], cfg["logo_height"]
    logo = cv.crop_rows(lh)
    colors = {logo[i:i + 3] for i in range(0, len(logo), 3)}
    if len(colors) > 224:
        sys.exit(f"el logo del kernel tiene {len(colors)} colores (máximo 224)")

    os.makedirs(a.outdir, exist_ok=True)
    outs = []
    if "png" in only:
        outs.append(("splash.png", png_bytes(w, h, bytes(cv.px))))
    if "ppm" in only:
        outs.append(("splash.ppm", ppm_p6(w, h, bytes(cv.px))))
    if "kernel" in only:
        outs.append(("logo_rgb20s_clut224.ppm", ppm_p3(w, lh, logo)))
    for name, data in outs:
        p = os.path.join(a.outdir, name)
        state = "actualizado" if write_if_changed(p, data) else "sin cambios"
        print(f"{p}: {state} ({len(data)} bytes)")
    print(f"logo del kernel: {w}x{lh}, {len(colors)} colores")


if __name__ == "__main__":
    main()
