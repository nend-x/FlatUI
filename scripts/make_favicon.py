#!/usr/bin/env python3
"""Generate FlatUI favicon.ico (16/32/48 px, BMP-encoded ICO, no deps).

Design: terracotta rounded square + cream 'F' glyph built from rectangles,
matching the FlatUI theme (--accent-terracotta #B8835A, --sand-cream #EDE4D3).
Output: src/public/favicon.ico (vite copies it to dist root automatically).
"""
import struct
import os

OUT = "/home/z/my-project/flatui-src/cream-shell/src/public/favicon.ico"

TERRACOTTA = (0x5A, 0x83, 0xB8)   # BGR of #B8835A
CREAM      = (0xD3, 0xE4, 0xED)   # BGR of #EDE4D3
DARK       = (0x21, 0x36, 0x4B)   # BGR of #4B3621 (fallback bg)
TRANSPARENT = (0, 0, 0, 0)
OPAQUE = lambda rgb: (rgb[0], rgb[1], rgb[2], 255)


def rounded_square(size, radius_frac=0.22):
    """Return 2D list [y][x] of RGBA pixels for a filled rounded square."""
    r = max(2, int(size * radius_frac))
    px = [[None] * size for _ in range(size)]
    for y in range(size):
        for x in range(size):
            # distance-based rounded rect test
            dx = min(x, size - 1 - x)
            dy = min(y, size - 1 - y)
            if dx >= r or dy >= r:
                inside = True
            else:
                # corner zone: circle test with anti-aliased edge
                cx, cy = r - dx, r - dy
                d = (cx * cx + cy * cy) ** 0.5
                inside = d <= r + 0.5
            px[y][x] = OPAQUE(TERRACOTTA) if inside else TRANSPARENT
    return px


def stamp_f(px, size):
    """Draw a minimal 'F' with cream bars, scaled to size."""
    def rect(x0, y0, x1, y1):
        for y in range(int(y0), int(y1)):
            for x in range(int(x0), int(x1)):
                if 0 <= x < size and 0 <= y < size:
                    px[y][x] = OPAQUE(CREAM)

    s = size
    # vertical stem
    rect(s * 0.30, s * 0.22, s * 0.40, s * 0.80)
    # top arm
    rect(s * 0.30, s * 0.22, s * 0.72, s * 0.32)
    # middle arm
    rect(s * 0.30, s * 0.46, s * 0.64, s * 0.56)


def bmp_entry(size):
    px = rounded_square(size)
    stamp_f(px, size)
    w = h = size
    xor = b""
    for y in range(h - 1, -1, -1):          # bottom-up rows
        for x in range(w):
            r, g, b, a = px[y][x]
            xor += struct.pack("<BBBB", b, g, r, a)
    and_stride = ((w + 31) // 32) * 4        # 1bpp rows, word aligned
    and_mask = b"\x00" * (and_stride * h)    # alpha channel handles transparency
    bmp_header = struct.pack(
        "<IiiHHIIiiII", 40, w, h * 2, 1, 32, 0, len(xor) + len(and_mask), 0, 0, 0, 0
    )
    return bmp_header + xor + and_mask


def make_ico(sizes=(16, 32, 48)):
    images = [bmp_entry(s) for s in sizes]
    count = len(sizes)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + 16 * count
    entries = b""
    for s, img in zip(sizes, images):
        b_or_zero = 0 if s >= 256 else s
        entries += struct.pack(
            "<BBBBHHII", b_or_zero, b_or_zero, 0, 0, 1, 32, len(img), offset
        )
        offset += len(img)
    return header + entries + b"".join(images)


os.makedirs(os.path.dirname(OUT), exist_ok=True)
data = make_ico()
with open(OUT, "wb") as f:
    f.write(data)
print(f"wrote {OUT} ({len(data)} bytes)")
