"""Convert canonical renderer PPM exports to PNG and a six-state review sheet (stdlib only).

Run after: cargo run -p kivori-golden-frames --example mascot_review
This only packages renderer output; it does not draw or animate the mascot.
"""
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets/compiled/review"


def png(path, width, height, pixels):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    rows = b"".join(b"\0" + pixels[y * width * 3:(y + 1) * width * 3] for y in range(height))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">2I5B", width, height, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def ppm(path):
    magic, size, depth, pixels = path.read_bytes().split(b"\n", 3)
    assert (magic, size, depth) == (b"P6", b"240 240", b"255")
    assert len(pixels) == 240 * 240 * 3
    return pixels


for source in OUT.glob("*.ppm"):
    png(source.with_suffix(".png"), 240, 240, ppm(source))

states = ["booting", "idle", "happy", "busy", "sleeping", "offline"]
sheet = bytearray(720 * 480 * 3)
for index, state in enumerate(states):
    pixels = ppm(OUT / f"{state}-600.ppm")
    for row in range(240):
        start = ((index // 3 * 240 + row) * 720 + index % 3 * 240) * 3
        sheet[start:start + 720] = pixels[row * 720:(row + 1) * 720]
png(OUT / "states.png", 720, 480, sheet)

idle_times = [20877, 21227, 21377, 21457, 22277]
idle_sheet = bytearray(1200 * 240 * 3)
for index, elapsed_ms in enumerate(idle_times):
    pixels = ppm(OUT / f"idle-life-{elapsed_ms}.ppm")
    for row in range(240):
        start = (row * 1200 + index * 240) * 3
        idle_sheet[start:start + 720] = pixels[row * 720:(row + 1) * 720]
png(OUT / "idle-life.png", 1200, 240, idle_sheet)
print(OUT / "states.png")
