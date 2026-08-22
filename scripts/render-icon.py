#!/usr/bin/env python3
"""Rasterise the BHUninstaller icon to a true-transparent PNG.

`qlmanage -t` renders an SVG onto a white card, so the corners outside the
rounded square came out opaque white — invisible on macOS, an obvious white
box on Windows. The artwork is pure geometry, so it is drawn here directly:
no ImageMagick, no browser, and the area outside the shape is genuinely empty.
"""
import struct, zlib, sys

SIZE = 1024
R = 228.0                      # corner radius of the tile


def lerp(a, b, t):
    return a + (b - a) * t


def gradient(y):
    """Vertical BiswasHost blue: #3b8bff -> #0d6efd at 55% -> #0948b3."""
    t = y / (SIZE - 1)
    if t < 0.55:
        k = t / 0.55
        return (lerp(0x3b, 0x0d, k), lerp(0x8b, 0x6e, k), lerp(0xff, 0xfd, k))
    k = (t - 0.55) / 0.45
    return (lerp(0x0d, 0x09, k), lerp(0x6e, 0x48, k), lerp(0xfd, 0xb3, k))


def rounded_coverage(px, py, x, y, w, h, r):
    """Antialiased coverage of a rounded rectangle at a pixel centre."""
    cx, cy = px + 0.5, py + 0.5
    # distance outside the rounded rect, negative inside
    dx = max(x + r - cx, 0.0, cx - (x + w - r))
    dy = max(y + r - cy, 0.0, cy - (y + h - r))
    if cx < x - 1 or cx > x + w + 1 or cy < y - 1 or cy > y + h + 1:
        return 0.0
    if dx == 0.0 and dy == 0.0:
        inside = min(cx - x, x + w - cx, cy - y, y + h - cy)
        return min(max(inside + 0.5, 0.0), 1.0)
    d = (dx * dx + dy * dy) ** 0.5 - r
    return min(max(0.5 - d, 0.0), 1.0)


# The app: a solid tile, and its leftovers breaking away underneath.
SHAPES = [
    (286, 212, 452, 326, 66, 1.00),
    (330, 608, 364, 52, 26, 0.95),
    (378, 700, 268, 52, 26, 0.66),
    (430, 792, 164, 52, 26, 0.38),
]

rows = []
for py in range(SIZE):
    gr, gg, gb = gradient(py)
    row = bytearray(b"\x00")            # PNG filter byte: none
    for px in range(SIZE):
        a = rounded_coverage(px, py, 0, 0, SIZE, SIZE, R)
        if a <= 0.0:
            row += b"\x00\x00\x00\x00"   # genuinely transparent
            continue
        r, g, b = gr, gg, gb
        for (x, y, w, h, rr, alpha) in SHAPES:
            c = rounded_coverage(px, py, x, y, w, h, rr) * alpha
            if c > 0.0:
                r = lerp(r, 255.0, c)
                g = lerp(g, 255.0, c)
                b = lerp(b, 255.0, c)
        row += bytes((int(r + 0.5), int(g + 0.5), int(b + 0.5), int(a * 255 + 0.5)))
    rows.append(bytes(row))

raw = b"".join(rows)


def chunk(tag, data):
    return (struct.pack(">I", len(data)) + tag + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))


png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(raw, 9))
       + chunk(b"IEND", b""))

out = sys.argv[1] if len(sys.argv) > 1 else "brand/icon-1024.png"
open(out, "wb").write(png)
print(f"wrote {out} ({len(png)//1024} KB)")
