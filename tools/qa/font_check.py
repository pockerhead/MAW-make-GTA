"""Every character the UI can print exists in every shipped UI font.

Scans the string literals of assets/ui/strings.ron and src/menu/*.rs and checks each
character against the cmap of the `font` and `title_font` files named in strings.ron.
A missing glyph renders as a box or nothing, and no headless gate sees it.

Usage: python tools/qa/font_check.py   (exit 1 on a missing glyph, 2 on a broken setup)
"""

import re
import sys
from pathlib import Path

from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "assets"
STRINGS = ASSETS / "ui" / "strings.ron"
LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')


def literals(path):
    text = path.read_text(encoding="utf-8")
    # Drop comments so prose in them is not checked as UI text.
    text = re.sub(r"//[^\n]*", "", text)
    return LITERAL.findall(text)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    ron = STRINGS.read_text(encoding="utf-8")
    fonts = {}
    for key in ("font", "title_font"):
        match = re.search(rf'\b{key}:\s*"([^"]+)"', ron)
        if not match:
            print(f"GATE BROKEN: {key} not found in {STRINGS}")
            return 2
        fonts[match.group(1)] = TTFont(ASSETS / match.group(1)).getBestCmap()

    sources = [STRINGS, *sorted((ROOT / "src" / "menu").glob("*.rs"))]
    texts = [(path, text) for path in sources for text in literals(path)]
    if len(texts) < 20:
        print(f"GATE BROKEN: only {len(texts)} string literals found")
        return 2

    missing = []
    for font, cmap in fonts.items():
        for path, text in texts:
            for ch in set(text):
                if ch.isspace() or ord(ch) in cmap:
                    continue
                missing.append(f"{font}: U+{ord(ch):04X} {ch!r} in {path.relative_to(ROOT)}: {text!r}")
    for line in sorted(missing):
        print(line)
    print(f"font_check: {len(texts)} literals, {len(fonts)} fonts, {len(missing)} missing glyphs")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
