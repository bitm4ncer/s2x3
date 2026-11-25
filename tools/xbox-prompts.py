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
import os
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

# The controls sheet in the US disc's texture set: 256x256, four 20x20 glyphs
# at y=123..142, x=12/34/56/78 (followed by two arrow glyphs).
# The game ships the same controls sheet under more than one texture id: the
# front end uses the first, the in-game pause/HUD path the second.
ATLASES = ["f897162c0f661c8b.png", "75df99fccd7d9d0b.png"]
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


def asset_glyph(size: int, letter: str, buttons: Path) -> Image.Image | None:
    """The premium button PNG for a letter, trimmed and scaled to the slot."""
    path = buttons / f"button_xbox_digital_{letter.lower()}_4.png"
    if not path.is_file():
        return None
    img = Image.open(path).convert("RGBA")
    bbox = img.getchannel("A").getbbox()
    if bbox:
        img = img.crop(bbox)
    return img.resize((size, size), Image.LANCZOS)


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


def patch(src: Path, buttons: Path | None = None) -> Image.Image:
    img = Image.open(src).convert("RGBA")
    for x0, y0, x1, y1, letter, colour in GLYPHS:
        w, h = x1 - x0 + 1, y1 - y0 + 1
        img.paste((0, 0, 0, 0), (x0, y0, x1 + 1, y1 + 1))
        glyph = asset_glyph(w, letter, buttons) if buttons else None
        img.alpha_composite(glyph or draw_glyph(w, letter, colour), (x0, y0))
    return img


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dump", required=True, help="texture dump directory")
    ap.add_argument("--out", required=True, help="texture replacement directory")
    ap.add_argument(
        "--buttons",
        default=os.environ.get("XBOX_BUTTONS_DIR", ""),
        help="folder with button_xbox_digital_<letter>_4.png (optional)",
    )
    ap.add_argument(
        "--atlas",
        action="append",
        dest="atlases",
        help="atlas file name (repeatable; default: both known ids)",
    )
    args = ap.parse_args()

    dump = Path(args.dump).expanduser()
    out = Path(args.out).expanduser()
    out.mkdir(parents=True, exist_ok=True)

    written = 0
    for name in args.atlases or ATLASES:
        src = dump / name
        if not src.is_file():
            print(f"skip {name} (not in the dump)", file=sys.stderr)
            continue
        dst = out / name
        patch(src, Path(args.buttons) if args.buttons else None).save(dst)
        print(f"wrote {dst}")
        written += 1

    if written == 0:
        print(
            f"none of the atlases are in {dump}\n"
            "Run the game once with SSX3_TEX_DUMP=<dir> and open a menu or pause.",
            file=sys.stderr,
        )
        return 2
    print(f"start the game with SSX3_TEX_REPLACE={out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
