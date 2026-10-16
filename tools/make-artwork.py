#!/usr/bin/env python3
"""Generates FramePlayerX's Steam library artwork into assets/steam/.

Sizes follow Steam's non-Steam-game artwork: portrait capsule 600x900,
hero 3840x1240, logo 1280x720 (transparent), wide capsule 920x430 and a
256x256 icon. The mark comes from assets/logo.svg and the wordmark is set
in Inter, the app's font. Needs Pillow and rsvg-convert (librsvg2-bin).
Re-run after design changes:
    python3 tools/make-artwork.py
"""
import io
import re
import subprocess
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "steam"
LOGO = ROOT / "assets" / "logo.svg"
FONT = ROOT / "crates" / "frameplayerx" / "assets" / "fonts" / "Inter-Bold.ttf"
TOP, BOTTOM = (20, 26, 51), (43, 26, 94)
BLUE, VIOLET = (92, 160, 255), (170, 110, 255)


def svg(text, size):
    png = subprocess.run(
        ["rsvg-convert", "-w", str(size), "-h", str(size)],
        input=text.encode(), capture_output=True, check=True,
    ).stdout
    return Image.open(io.BytesIO(png)).convert("RGBA")


def icon(size):
    """The app icon: the mark on its rounded square."""
    return svg(LOGO.read_text(), size)


def mark(size):
    """The mark alone, on transparency."""
    return svg(re.sub(r'<rect id="background"[^>]*/>', "", LOGO.read_text()), size)


def gradient(w, h):
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        t = y / max(h - 1, 1)
        row = tuple(int(TOP[i] + (BOTTOM[i] - TOP[i]) * t) for i in range(3))
        for x in range(w):
            px[x, y] = row
    return img


def glow(img, cx, cy, r, color):
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    ImageDraw.Draw(layer).ellipse((cx - r, cy - r, cx + r, cy + r), fill=color + (90,))
    layer = layer.filter(ImageFilter.GaussianBlur(r * 0.6))
    img.alpha_composite(layer)


def canvas(w, h):
    img = gradient(w, h).convert("RGBA")
    glow(img, int(w * 0.75), int(h * 0.3), int(min(w, h) * 0.45), VIOLET)
    glow(img, int(w * 0.2), int(h * 0.8), int(min(w, h) * 0.4), BLUE)
    return img


def place(img, m, cx, cy):
    img.alpha_composite(m, (int(cx - m.width / 2), int(cy - m.height / 2)))


def wordmark(img, x, y, height, centered=False):
    """"FramePlayer" in white and the "X" in the mark's blue-to-violet,
    vertically centred on `y`, starting at `x` (or centred on it)."""
    font = ImageFont.truetype(str(FONT), int(height))
    d = ImageDraw.Draw(img)
    base = d.textlength("FramePlayer", font=font)
    total = d.textlength("FramePlayerX", font=font)
    if centered:
        x -= total / 2
    d.text((x, y), "FramePlayer", font=font, fill=(255, 255, 255, 255), anchor="lm")
    # The X: its shape as a mask over a left-to-right gradient.
    left, top, right, bottom = (int(v) for v in d.textbbox((x + base, y), "X", font=font, anchor="lm"))
    w, h = right - left, bottom - top
    mask = Image.new("L", img.size, 0)
    ImageDraw.Draw(mask).text((x + base, y), "X", font=font, fill=255, anchor="lm")
    fill = Image.new("RGBA", img.size, (0, 0, 0, 0))
    px = fill.load()
    for i in range(w):
        t = i / max(w - 1, 1)
        c = tuple(int(BLUE[k] + (VIOLET[k] - BLUE[k]) * t) for k in range(3)) + (255,)
        for j in range(h):
            if 0 <= left + i < img.width and 0 <= top + j < img.height:
                px[left + i, top + j] = c
    img.paste(fill, (0, 0), mask)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    p = canvas(600, 900)
    place(p, mark(440), 300, 370)
    wordmark(p, 300, 690, 70, centered=True)
    ImageDraw.Draw(p).text(
        (300, 775), "VR video player", font=ImageFont.truetype(str(FONT), 30),
        fill=(190, 196, 215, 255), anchor="mm",
    )
    p.convert("RGB").save(OUT / "portrait.png")

    c = canvas(920, 430)
    place(c, mark(270), 165, 215)
    wordmark(c, 305, 215, 72)
    c.convert("RGB").save(OUT / "capsule.png")

    h = canvas(3840, 1240)
    place(h, mark(1000), 2900, 620)
    h.convert("RGB").save(OUT / "hero.png")

    logo = Image.new("RGBA", (1280, 720), (0, 0, 0, 0))
    place(logo, mark(340), 200, 360)
    wordmark(logo, 360, 360, 112)
    logo.save(OUT / "logo.png")

    icon(256).save(OUT / "icon.png")
    for f in sorted(OUT.iterdir()):
        print(f, Image.open(f).size)


if __name__ == "__main__":
    main()
