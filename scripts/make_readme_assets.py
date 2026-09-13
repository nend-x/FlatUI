#!/usr/bin/env python3
"""Generate FlatUI GitHub README assets: banner.png (1280x320) + icon.png (512x512).

Brand palette (matches the app theme):
  espresso   #2A1E12 (background)
  terracotta #B8835A (accent / logo tile)
  cream      #EDE4D3 (text)
"""
from PIL import Image, ImageDraw, ImageFont

TERRACOTTA = (184, 131, 90)
CREAM = (237, 228, 211)
ESPRESSO_TOP = (48, 34, 22)
ESPRESSO_BOT = (30, 21, 13)
MUTED = (168, 140, 108)

BOLD = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
REG = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"


def vgrad(w: int, h: int, top, bot) -> Image.Image:
    img = Image.new("RGB", (w, h))
    for y in range(h):
        t = y / (h - 1)
        c = tuple(int(top[i] + (bot[i] - top[i]) * t) for i in range(3))
        ImageDraw.Draw(img).line([(0, y), (w, y)], fill=c)
    return img


def rounded_tile(size: int, radius: int, color) -> Image.Image:
    """Solid rounded square with a soft two-tone: slight top highlight."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=color + (255,))
    # subtle top highlight band (flat-design two-tone)
    hi = tuple(min(255, c + 18) for c in color) + (255,)
    d.rounded_rectangle([0, 0, size - 1, size // 2 - size // 22], radius=radius,
                        fill=hi)
    d.rectangle([0, size // 2 - size // 22, size - 1, size // 2 - 1], fill=hi)
    return img


def draw_f_glyph(canvas: Image.Image, box: tuple, color=CREAM):
    """Hand-drawn geometric 'F' glyph, flat style."""
    x0, y0, x1, y1 = box
    w = x1 - x0
    s = w / 100.0  # stroke unit
    d = ImageDraw.Draw(canvas)
    # vertical stem
    d.rounded_rectangle([x0, y0, x0 + 14 * s, y1], radius=3 * s, fill=color)
    # top bar
    d.rounded_rectangle([x0, y0, x0 + 62 * s, y0 + 14 * s], radius=3 * s, fill=color)
    # middle bar
    d.rounded_rectangle([x0, y0 + 36 * s, x0 + 48 * s, y0 + 50 * s], radius=3 * s, fill=color)


def make_icon(path: str, size=512):
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    tile = rounded_tile(size, int(size * 0.23), TERRACOTTA)
    img.alpha_composite(tile)
    draw_f_glyph(img, (size * 0.31, size * 0.19, size * 0.85, size * 0.81))
    img.save(path)
    print("wrote", path)


def make_banner(path: str, W=1280, H=320):
    img = vgrad(W, H, ESPRESSO_TOP, ESPRESSO_BOT).convert("RGBA")
    d = ImageDraw.Draw(img)

    # faint oversized watermark tile on the right
    wm = rounded_tile(360, 82, (TERRACOTTA[0], TERRACOTTA[1], TERRACOTTA[2]))
    mask = wm.split()[3].point(lambda a: a // 12)
    wm.putalpha(mask)
    img.alpha_composite(wm, (W - 430, -60))
    wm2 = wm.copy()
    img.alpha_composite(wm2, (W - 250, 120))

    # logo tile + F
    ts = 200
    tile = rounded_tile(ts, int(ts * 0.23), TERRACOTTA)
    img.alpha_composite(tile, (64, 60))
    draw_f_glyph(img, (64 + ts * 0.29, 60 + ts * 0.235, 64 + ts * 0.73, 60 + ts * 0.765))

    # wordmark
    f_title = ImageFont.truetype(BOLD, 92)
    f_sub = ImageFont.truetype(REG, 27)
    f_tag = ImageFont.truetype(BOLD, 21)
    x = 312
    d.text((x, 118), "FlatUI", font=f_title, fill=CREAM, anchor="lm")

    # divider + subtitle
    d.rounded_rectangle([x, 182, x + 260, 189], radius=3, fill=TERRACOTTA)
    d.text((x, 222), "Flat, minimal shell replacement for Windows",
           font=f_sub, fill=(205, 190, 168), anchor="lm")
    d.text((x, 258), "custom taskbar  •  Spotlight-style launcher  •  Tauri 2 + Rust",
           font=f_sub, fill=MUTED, anchor="lm")
    d.text((x, 288), "v0.1.0", font=f_tag, fill=TERRACOTTA, anchor="lm")

    img.convert("RGB").save(path, "PNG")
    print("wrote", path)


if __name__ == "__main__":
    import sys
    out = sys.argv[1] if len(sys.argv) > 1 else "."
    make_banner(f"{out}/banner.png")
    make_icon(f"{out}/icon.png")
