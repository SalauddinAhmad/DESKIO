#!/usr/bin/env python3
"""Generate every BHUninstaller icon from the artwork, with real transparency.

Two lessons are baked in here:

1. `qlmanage -t` renders an SVG onto a WHITE CARD, so anything rasterised that
   way carries a white box. The artwork is pure geometry and is drawn directly
   instead — no ImageMagick, no browser.

2. Windows accepts PNG-compressed .ico entries, but several shell surfaces (the
   taskbar and the small title-bar icon among them) composite them onto white
   at 16/32/48. Small sizes are therefore written as uncompressed 32-bit BMP
   with an AND mask, which every shell handles correctly. Only 256 stays PNG,
   where it is both required and safe.

Usage:  python3 scripts/make-icons.py
"""
import struct, zlib, pathlib, subprocess, shutil, sys

OUT = pathlib.Path("app/src-tauri/icons")
BRAND = pathlib.Path("brand")
R_RATIO = 228.0 / 1024.0           # corner radius as a fraction of the tile

# The app tile, then its leftovers breaking away underneath, as fractions of
# the canvas so every size is drawn rather than downscaled.
SHAPES = [
    (286, 212, 452, 326, 66, 1.00),
    (330, 608, 364, 52, 26, 0.95),
    (378, 700, 268, 52, 26, 0.66),
    (430, 792, 164, 52, 26, 0.38),
]


def lerp(a, b, t):
    return a + (b - a) * t


def gradient(y, size):
    t = y / (size - 1) if size > 1 else 0.0
    if t < 0.55:
        k = t / 0.55
        return (lerp(0x3B, 0x0D, k), lerp(0x8B, 0x6E, k), lerp(0xFF, 0xFD, k))
    k = (t - 0.55) / 0.45
    return (lerp(0x0D, 0x09, k), lerp(0x6E, 0x48, k), lerp(0xFD, 0xB3, k))


def coverage(cx, cy, x, y, w, h, r):
    """Antialiased coverage of a rounded rectangle at a point."""
    if cx < x - 1 or cx > x + w + 1 or cy < y - 1 or cy > y + h + 1:
        return 0.0
    dx = max(x + r - cx, 0.0, cx - (x + w - r))
    dy = max(y + r - cy, 0.0, cy - (y + h - r))
    if dx == 0.0 and dy == 0.0:
        inside = min(cx - x, x + w - cx, cy - y, y + h - cy)
        return min(max(inside + 0.5, 0.0), 1.0)
    d = (dx * dx + dy * dy) ** 0.5 - r
    return min(max(0.5 - d, 0.0), 1.0)


def render(size):
    """Rows of RGBA bytes, top-down."""
    s = size / 1024.0
    radius = R_RATIO * size
    shapes = [(x * s, y * s, w * s, h * s, r * s, a) for (x, y, w, h, r, a) in SHAPES]
    rows = []
    for py in range(size):
        gr, gg, gb = gradient(py, size)
        row = bytearray()
        cy = py + 0.5
        for px in range(size):
            cx = px + 0.5
            a = coverage(cx, cy, 0, 0, size, size, radius)
            if a <= 0.0:
                row += b"\x00\x00\x00\x00"
                continue
            r, g, b = gr, gg, gb
            for (x, y, w, h, rr, alpha) in shapes:
                c = coverage(cx, cy, x, y, w, h, rr) * alpha
                if c > 0.0:
                    r, g, b = lerp(r, 255.0, c), lerp(g, 255.0, c), lerp(b, 255.0, c)
            row += bytes((int(r + .5), int(g + .5), int(b + .5), int(a * 255 + .5)))
        rows.append(bytes(row))
    return rows


def write_png(rows, size, path):
    raw = b"".join(b"\x00" + r for r in rows)

    def chunk(tag, data):
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    path.write_bytes(b"\x89PNG\r\n\x1a\n"
                     + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(raw, 9))
                     + chunk(b"IEND", b""))
    return path.read_bytes()


def bmp_entry(rows, size):
    """A 32-bit BGRA DIB with an AND mask — the format Windows shells expect."""
    xor = bytearray()
    for y in range(size - 1, -1, -1):           # DIBs are stored bottom-up
        row = rows[y]
        for x in range(size):
            r, g, b, a = row[x*4:x*4+4]
            xor += bytes((b, g, r, a))

    mask_stride = ((size + 31) // 32) * 4
    mask = bytearray()
    for y in range(size - 1, -1, -1):
        bits = bytearray(mask_stride)
        for x in range(size):
            if rows[y][x*4+3] == 0:             # 1 = fully transparent
                bits[x >> 3] |= 0x80 >> (x & 7)
        mask += bits

    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0,
                         len(xor) + len(mask), 0, 0, 0, 0)
    return header + bytes(xor) + bytes(mask)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    BRAND.mkdir(parents=True, exist_ok=True)

    rendered = {}
    for size in (16, 32, 48, 64, 128, 256, 512, 1024):
        rendered[size] = render(size)
        print(f"  rendered {size}x{size}")

    write_png(rendered[1024], 1024, BRAND / "icon-1024.png")
    for size, name in ((32, "32x32.png"), (128, "128x128.png"),
                       (256, "128x128@2x.png"), (256, "256x256.png"),
                       (512, "512x512.png"), (512, "icon.png")):
        write_png(rendered[size], size, OUT / name)

    # .ico — BMP for the sizes the shell draws small, PNG only at 256.
    entries, payload = b"", b""
    blobs = []
    for size in (16, 32, 48, 64, 128):
        blobs.append((size, bmp_entry(rendered[size], size)))
    tmp = pathlib.Path("/tmp/bhu-icon-256.png")
    blobs.append((256, write_png(rendered[256], 256, tmp)))
    tmp.unlink(missing_ok=True)

    offset = 6 + 16 * len(blobs)
    for size, data in blobs:
        entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32,
                               len(data), offset)
        payload += data
        offset += len(data)
    (OUT / "icon.ico").write_bytes(struct.pack("<HHH", 0, 1, len(blobs)) + entries + payload)
    print(f"  icon.ico: {len(blobs)} entries, BMP below 256")

    # .icns, on macOS only — iconutil is not available elsewhere.
    if shutil.which("iconutil"):
        iset = pathlib.Path("/tmp/BHUninstaller.iconset")
        if iset.exists():
            shutil.rmtree(iset)
        iset.mkdir()
        for size, name in ((16, "icon_16x16"), (32, "icon_16x16@2x"), (32, "icon_32x32"),
                           (64, "icon_32x32@2x"), (128, "icon_128x128"), (256, "icon_128x128@2x"),
                           (256, "icon_256x256"), (512, "icon_256x256@2x"),
                           (512, "icon_512x512"), (1024, "icon_512x512@2x")):
            write_png(rendered[size], size, iset / f"{name}.png")
        subprocess.run(["iconutil", "-c", "icns", str(iset), "-o", str(OUT / "icon.icns")],
                       check=True)
        print("  icon.icns written")

    print("done")


if __name__ == "__main__":
    sys.exit(main())
