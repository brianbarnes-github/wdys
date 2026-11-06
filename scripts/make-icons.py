#!/usr/bin/env python3
"""Builds the Windows icon assets from resources/wdys-icon.png.

Outputs (committed, so the Rust build needs no image tooling):
  resources/wdys.ico       multi-size icon embedded in wdys.exe
  resources/tray-32.rgba   raw 32x32 RGBA bytes used for the tray icon
Requires Pillow.  usage: python3 scripts/make-icons.py
"""
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "resources" / "wdys-icon.png"
SIZES = [16, 24, 32, 48, 64, 128, 256]
SUPERSAMPLE = 4
CORNER_RADIUS = 0.215  # fraction of the tile width; matches the artwork's rounded square
EDGE_INSET = 0.018  # trims leftover white backdrop and shadow from the tile edge


def tile_bounds(img: Image.Image) -> tuple[int, int, int, int]:
    """Bounding box of the saturated blue tile, ignoring the white backdrop and soft shadow."""
    r, _, b = img.split()
    blue = ImageChops.subtract(b, r).point(lambda v: 255 if v > 70 else 0)
    box = blue.getbbox()
    if box is None:
        raise SystemExit("could not find the icon tile in the image")
    return box


def transparent_tile(img: Image.Image) -> Image.Image:
    left, top, right, bottom = tile_bounds(img)
    side = max(right - left, bottom - top)
    cx, cy = (left + right) // 2, (top + bottom) // 2
    box = (cx - side // 2, cy - side // 2, cx - side // 2 + side, cy - side // 2 + side)
    inset = int(side * EDGE_INSET)
    box = (box[0] + inset, box[1] + inset, box[2] - inset, box[3] - inset)
    side = box[2] - box[0]
    tile = img.crop(box).convert("RGBA")
    big = side * SUPERSAMPLE
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, big - 1, big - 1), radius=int(big * CORNER_RADIUS), fill=255)
    tile.putalpha(mask.resize((side, side), Image.LANCZOS))
    return tile


def main() -> None:
    master = transparent_tile(Image.open(SRC).convert("RGB"))
    print(f"tile {master.size[0]}x{master.size[1]}")
    master.save(ROOT / "resources" / "wdys.ico", sizes=[(s, s) for s in SIZES])
    (ROOT / "resources" / "tray-32.rgba").write_bytes(master.resize((32, 32), Image.LANCZOS).tobytes())


if __name__ == "__main__":
    main()
