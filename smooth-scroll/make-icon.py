#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow"]
# ///
"""Make the "Ghostty Smooth" app icon from Ghostty's icon.

Shifts the icon's hue toward magenta and adds a round badge with three
waves in the bottom-right corner, so the fork is easy to tell apart from
the official Ghostty in the Dock and app switcher.

Usage: make-icon.py IN.icns OUT.icns
"""

import math
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw

HUE_SHIFT = 0.42  # fraction of the color wheel
BADGE_BG = (198, 120, 221, 255)  # One Dark magenta
BADGE_FG = (255, 255, 255, 255)


def shift_hue(img: Image.Image) -> Image.Image:
    alpha = img.getchannel("A")
    h, s, v = img.convert("RGB").convert("HSV").split()
    offset = int(HUE_SHIFT * 255)
    h = h.point(lambda x: (x + offset) % 256)
    out = Image.merge("HSV", (h, s, v)).convert("RGB").convert("RGBA")
    out.putalpha(alpha)
    return out


def add_badge(img: Image.Image) -> Image.Image:
    w, _ = img.size
    # Draw at 4x and downsample for smooth edges.
    scale = 4
    size = int(w * 0.38) * scale
    badge = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(badge)
    d.ellipse((0, 0, size - 1, size - 1), fill=BADGE_BG)
    line = max(scale, size // 14)
    for i in range(3):
        cy = size * (0.33 + 0.17 * i)
        pts = [
            (x, cy + math.sin(x / size * 2 * math.pi * 1.5) * size * 0.05)
            for x in range(int(size * 0.2), int(size * 0.8) + 1, max(1, scale))
        ]
        d.line(pts, fill=BADGE_FG, width=line, joint="curve")
    badge = badge.resize((size // scale, size // scale), Image.LANCZOS)
    out = img.copy()
    margin = int(w * 0.06)
    out.alpha_composite(badge, (w - badge.width - margin, w - badge.height - margin))
    return out


def main() -> None:
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "icon.iconset"
        subprocess.run(["iconutil", "-c", "iconset", str(src), "-o", str(iconset)], check=True)
        for png in iconset.glob("*.png"):
            img = Image.open(png).convert("RGBA")
            add_badge(shift_hue(img)).save(png)
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(dst)], check=True)


if __name__ == "__main__":
    main()
