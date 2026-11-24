#!/usr/bin/env python3
"""Turn the game's PlayStation button glyphs into Xbox glyphs.

The prompts are cropped from one UI texture (the controls sheet). This tool
reads that texture from your own texture dump and writes a replacement with
A/B/X/Y drawn over the four PlayStation symbols. It contains no game art: the
input is your dump, the output stays on your machine.

Usage:
    # one run with the dump on, reach a menu with prompts, then quit
    SSX3_TEX_DUMP=~/Games/SSX3-ps2/texdump ./ssx3

    python3 tools/xbox-prompts.py \
        --dump ~/Games/SSX3-ps2/texdump \
        --out  ~/Games/SSX3-ps2/texreplace

    SSX3_TEX_REPLACE=~/Games/SSX3-ps2/texreplace ./ssx3

Mapping: Triangle=Y, Square=X, Cross=A, Circle=B (positional, like the
engine's pad mapping).
"""

import argparse
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

# The controls sheet in the US disc's texture set: 256x256, four 20x20 glyphs
# at y=123..142, x=12/34/56/78 (followed by two arrow glyphs).
ATLAS = "f897162c0f661c8b.png"
GLYPHS = [
    (12, 123, 31, 142, "Y", "#f2c744"),  # was Triangle
    (34, 123, 53, 142, "X", "#3b82f6"),  # was Square
    (56, 123, 75, 142, "A", "#3fbf5f"),  # was Cross
    (78, 123, 97, 142, "B", "#e5484d"),  # was Circle
]

FONT_CANDIDATES = [
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "C:/Windows/Fonts/arialbd.ttf",
]


def load_font(size: int):
    for path in FONT_CANDIDATES:
        if Path(path).is_file():
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def draw_glyph(size: int, letter: str, colour: str) -> Image.Image:
    scale = 8
    tile = Image.new("RGBA", (size * scale, size * scale), (0, 0, 0, 0))
    d = ImageDraw.Draw(tile)
    d.ellipse((0, 0, size * scale - 1, size * scale - 1), fill=colour)
    font = load_font(int(size * scale * 0.78))
    box = d.textbbox((0, 0), letter, font=font)
    d.text(
        (
            (size * scale - (box[2] - box[0])) / 2 - box[0],
            (size * scale - (box[3] - box[1])) / 2 - box[1],
        ),
        letter,
        font=font,
        fill="white",
    )
    return tile.resize((size, size), Image.LANCZOS)


def patch(src: Path) -> Image.Image:
    img = Image.open(src).convert("RGBA")
    for x0, y0, x1, y1, letter, colour in GLYPHS:
        w, h = x1 - x0 + 1, y1 - y0 + 1
        img.paste((0, 0, 0, 0), (x0, y0, x1 + 1, y1 + 1))
        img.alpha_composite(draw_glyph(w, letter, colour), (x0, y0))
    return img


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dump", required=True, help="texture dump directory")
    ap.add_argument("--out", required=True, help="texture replacement directory")
    ap.add_argument("--atlas", default=ATLAS, help=f"atlas file name (default {ATLAS})")
    args = ap.parse_args()

    src = Path(args.dump).expanduser() / args.atlas
    if not src.is_file():
        print(f"missing {src}\nRun the game once with SSX3_TEX_DUMP=<dir> and open a menu.", file=sys.stderr)
        return 2

    out = Path(args.out).expanduser()
    out.mkdir(parents=True, exist_ok=True)
    dst = out / args.atlas
    patch(src).save(dst)
    print(f"wrote {dst}")
    print(f"start the game with SSX3_TEX_REPLACE={out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
