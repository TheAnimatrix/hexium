#!/usr/bin/env python3
"""Draw Hexium's icons and write them over Hex's icon files.

    python3 -m pip install pillow
    python3 hexium/branding/make_icons.py

The glyph is a pointy-top hexagon outline holding five rounded waveform bars
(voice), white on transparent. The app icon puts it on a saffron-to-purple
rounded square. Outputs, all at the paths Hex's build already reads:

    resources/windows/hex.ico           installer + exe icon (16-256 px)
    resources/windows/icon-{32,64,128}.png   window icons (white glyph)
    resources/windows/tray-32.rgba      tray icon, raw 32x32 RGBA (white glyph)
    app/AppIcon.icon/Assets/Image.png   macOS Icon Composer layer (white glyph)
    app/AppIcon.icon/icon.json          macOS background gradient
"""
import json
import math
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
TOP = (255, 153, 51)      # saffron
BOTTOM = (106, 27, 154)   # deep purple
SS = 4                    # supersampling factor


def hexagon(cx, cy, r):
    return [(cx + r * math.cos(math.radians(90 + 60 * i)), cy + r * math.sin(math.radians(90 + 60 * i)))
            for i in range(6)]


def glyph(w: int, h: int) -> Image.Image:
    """White hexagon + waveform on transparent, fitted to w x h."""
    W, H = w * SS, h * SS
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    r = min(W / (2 * math.cos(math.radians(30))), H / 2) * 0.96
    cx, cy = W / 2, H / 2
    stroke = max(SS, r * 0.13)
    pts = hexagon(cx, cy, r - stroke / 2)
    d.line(pts + [pts[0]], fill="white", width=round(stroke), joint="curve")
    for x, y in pts:  # round the corners
        d.ellipse([x - stroke / 2, y - stroke / 2, x + stroke / 2, y + stroke / 2], fill="white")
    bar_w = r * 0.13
    gap = r * 0.09
    heights = [0.34, 0.66, 1.0, 0.66, 0.34]
    total = len(heights) * bar_w + (len(heights) - 1) * gap
    x0 = cx - total / 2
    for i, hf in enumerate(heights):
        bh = r * 0.95 * hf
        x = x0 + i * (bar_w + gap)
        d.rounded_rectangle([x, cy - bh / 2, x + bar_w, cy + bh / 2], radius=bar_w / 2, fill="white")
    return im.resize((w, h), Image.LANCZOS)


def app_icon(size: int) -> Image.Image:
    S = size * SS
    grad = Image.new("RGBA", (S, S))
    px = grad.load()
    for y in range(S):
        t = y / (S - 1)
        c = tuple(round(TOP[k] + (BOTTOM[k] - TOP[k]) * t) for k in range(3)) + (255,)
        for x in range(S):
            px[x, y] = c
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=S * 0.225, fill=255)
    out = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    out.paste(grad, (0, 0), mask)
    g = glyph(round(size * 0.62), round(size * 0.62)).resize((round(S * 0.62),) * 2, Image.LANCZOS)
    out.alpha_composite(g, ((S - g.width) // 2, (S - g.height) // 2))
    return out.resize((size, size), Image.LANCZOS)


def main() -> None:
    win = ROOT / "resources" / "windows"
    big = app_icon(256)
    big.save(win / "hex.ico", sizes=[(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 256)])
    for s in (32, 64, 128):
        glyph(s, s).save(win / f"icon-{s}.png")
    (win / "tray-32.rgba").write_bytes(glyph(32, 32).tobytes())

    mac = ROOT / "app" / "AppIcon.icon"
    glyph(1488, 1672).save(mac / "Assets" / "Image.png")
    spec = json.loads((mac / "icon.json").read_text())
    srgb = lambda c: "srgb:" + ",".join(f"{v / 255:.5f}" for v in c) + ",1.00000"
    spec["fill"]["linear-gradient"] = [srgb(TOP), srgb(BOTTOM)]
    spec["fill"]["orientation"]["stop"]["y"] = 1.0
    spec["groups"][0]["layers"][0]["name"] = "Hexium"
    (mac / "icon.json").write_text(json.dumps(spec, indent=2) + "\n")
    big.save(ROOT / "hexium" / "branding" / "hexium-256.png")
    print("wrote Hexium icons")


if __name__ == "__main__":
    main()
