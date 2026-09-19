"""Renders the Parla app icon and menu bar template icon.

The mark is a lowercase "p" whose bowl doubles as a speech bubble holding voice
bars. Run from the project root, then `pnpm tauri icon design/icon-source.png`.
"""

import math
import pathlib
import struct
import zlib


def write_png(path, width, height, pixel):
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            rows.extend(pixel(x + 0.5, y + 0.5))

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
    png += chunk(b"IDAT", zlib.compress(bytes(rows), 9)) + chunk(b"IEND", b"")
    pathlib.Path(path).write_bytes(png)


def clamp(value, low=0.0, high=1.0):
    return min(max(value, low), high)


def coverage(distance):
    return clamp(0.5 - distance)


def rounded_rect(px, py, cx, cy, half, radius):
    qx, qy = abs(px - cx) - half + radius, abs(py - cy) - half + radius
    return math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - radius


def segment(px, py, x, y0, y1, half_width):
    return math.hypot(px - x, py - clamp(py, y0, y1)) - half_width


# Glyph geometry on a 24-unit grid, matching the SVG logo in src/app/ui.tsx.
def glyph(px, py, size, scale):
    u = (px - size / 2) / scale + 12
    v = (py - size / 2) / scale + 12
    bowl = abs(math.hypot(u - 12, v - 9.75) - 5.5) - 1.3
    stem = segment(u, v, 6.5, 9.75, 19.75, 1.3)
    bars = min(
        segment(u, v, 10, 8.65, 10.85, 0.75),
        segment(u, v, 12, 7.55, 11.95, 0.75),
        segment(u, v, 14, 8.65, 10.85, 0.75),
    )
    return min(bowl, stem, bars) * scale


SIZE, BODY, RADIUS, SCALE = 1024, 824, 185, 27.0
TOP = (SIZE - BODY) / 2


def app_icon(px, py):
    edge = rounded_rect(px, py, SIZE / 2, SIZE / 2, BODY / 2, RADIUS)
    body = coverage(edge)
    if body <= 0:
        return (0, 0, 0, 0)

    t = (py - TOP) / BODY
    glow = clamp(1 - math.hypot(px - SIZE * 0.34, py - TOP) / 620) ** 2
    shade = 46 - 32 * t + 16 * glow
    shade += 22 * clamp(1 + edge / 3) * clamp(1 - t * 3)  # hairline highlight on the top edge

    shadow = clamp(1 - max(glyph(px, py - 16, SIZE, SCALE), 0) / 46) ** 2
    shade *= 1 - 0.5 * shadow

    ink = coverage(glyph(px, py, SIZE, SCALE))
    glyph_tone = 252 - 26 * clamp((py - 250) / 520)
    v = int(round(shade + (glyph_tone - shade) * ink))
    return (v, v, v, int(round(255 * body)))


def tray_icon(px, py):
    return (0, 0, 0, int(round(255 * coverage(glyph(px, py, 44, 1.72)))))


# Same mark with a dot badge in the corner, shown while a recording is running. The
# glyph is cut away around the dot so the two never blur together at menu bar size.
DOT_X, DOT_Y, DOT_R, DOT_GAP = 35.5, 8.5, 6.5, 2.5


def tray_recording_icon(px, py):
    dot = math.hypot(px - DOT_X, py - DOT_Y) - DOT_R
    mark = max(glyph(px, py, 44, 1.72), -(dot - DOT_GAP))
    return (0, 0, 0, int(round(255 * max(coverage(mark), coverage(dot)))))


if __name__ == "__main__":
    write_png("design/icon-source.png", SIZE, SIZE, app_icon)
    write_png("src-tauri/icons/tray-template.png", 44, 44, tray_icon)
    write_png("src-tauri/icons/tray-recording-template.png", 44, 44, tray_recording_icon)
