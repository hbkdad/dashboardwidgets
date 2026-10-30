"""Generates the bundled starter wallpapers (original, procedural; no third-party art). Run: python scripts/make-bundled-wallpapers.py
Output: plinth/wallpaper-images/*.jpg (2560x1440). Deterministic: the same seeds produce the same images."""
import os, random, sys
from PIL import Image, ImageDraw, ImageFilter, ImageChops

W, H = 2560, 1440
OUT = os.path.join(os.path.dirname(__file__), "..", "plinth", "wallpaper-images")
os.makedirs(OUT, exist_ok=True)

PALETTES = {
    "Dusk Bloom": ((43, 27, 77), (217, 102, 122), [(255, 178, 122), (120, 60, 160), (255, 120, 150)]),
    "Graphite Mint": ((13, 15, 20), (44, 51, 71), [(124, 242, 200), (60, 90, 140), (30, 140, 120)]),
    "Paper Sky": ((233, 228, 218), (183, 196, 214), [(255, 255, 255), (150, 180, 230), (240, 200, 180)]),
    "Ember Glow": ((20, 8, 8), (120, 30, 20), [(255, 140, 40), (255, 80, 60), (90, 20, 40)]),
    "Deep Ocean": ((4, 18, 40), (10, 80, 120), [(40, 200, 220), (20, 100, 200), (120, 240, 240)]),
    "Forest Mist": ((12, 30, 24), (60, 110, 90), [(170, 230, 190), (40, 120, 90), (220, 240, 200)]),
    "Violet Night": ((16, 8, 36), (70, 30, 120), [(190, 120, 255), (90, 60, 200), (255, 120, 220)]),
    "Solar Flare": ((30, 14, 4), (200, 100, 20), [(255, 220, 90), (255, 140, 30), (200, 50, 30)]),
    "Arctic Light": ((200, 220, 235), (120, 160, 200), [(255, 255, 255), (170, 210, 245), (210, 190, 240)]),
    "Rose Quartz": ((60, 30, 50), (200, 130, 150), [(255, 200, 210), (190, 90, 130), (255, 230, 220)]),
    "Midnight Teal": ((6, 20, 26), (14, 70, 80), [(60, 220, 200), (20, 110, 140), (140, 255, 230)]),
    "Slate Gold": ((24, 26, 32), (74, 80, 94), [(235, 190, 90), (150, 120, 60), (200, 210, 230)]),
}

def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))

def diagonal(c1, c2):
    # bilinear blend of four corner colors: top-left c1, bottom-right c2, the other corners halfway (no hard edges)
    mid = lerp(c1, c2, 0.5)
    small = Image.new("RGB", (2, 2))
    small.putpixel((0, 0), c1); small.putpixel((1, 0), mid); small.putpixel((0, 1), mid); small.putpixel((1, 1), c2)
    return small.resize((W, H), Image.BILINEAR)

for idx, (name, (c1, c2, accents)) in enumerate(PALETTES.items()):
    rnd = random.Random(1000 + idx)
    img = diagonal(c1, c2)
    layer = Image.new("RGB", (W, H), (0, 0, 0))
    d = ImageDraw.Draw(layer)
    for k in range(5):
        col = accents[k % len(accents)]
        cx, cy = rnd.randint(0, W), rnd.randint(0, H)
        rx, ry = rnd.randint(300, 900), rnd.randint(250, 700)
        d.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], fill=tuple(int(c * rnd.uniform(0.35, 0.75)) for c in col))
    layer = layer.filter(ImageFilter.GaussianBlur(160))
    img = ImageChops.screen(img, layer)
    noise = Image.effect_noise((W, H), 18).convert("RGB")
    img = Image.blend(img, ImageChops.overlay(img, noise), 0.10)
    fn = name.lower().replace(" ", "-") + ".jpg"
    img.save(os.path.join(OUT, fn), "JPEG", quality=88, optimize=True)
    print(fn, os.path.getsize(os.path.join(OUT, fn)) // 1024, "KB")
