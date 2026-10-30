"""Creates a folder of test pictures for the library tests. Usage: python scripts/make-library-fixture.py <dir> [count]
Contents: <count> small unique PNG/JPG files spread over nested folders, one byte-identical duplicate, one corrupt .jpg,
one text file, one picture inside a folder named 'skipme' (for exclusion tests). Deterministic."""
import os, random, shutil, sys
from PIL import Image

out = sys.argv[1]
n = int(sys.argv[2]) if len(sys.argv) > 2 else 30
shutil.rmtree(out, ignore_errors=True)
os.makedirs(out)
rnd = random.Random(7)
for i in range(n):
    sub = os.path.join(out, f"set{i % 5}", "deep" if i % 7 == 0 else "")
    os.makedirs(sub, exist_ok=True)
    w, h = rnd.choice([(1600, 900), (1920, 1080), (1200, 1200), (800, 1200), (2560, 1080)]) if n <= 300 else (64, 36)
    img = Image.new("RGB", (w, h), (rnd.randint(0, 255), rnd.randint(0, 255), rnd.randint(0, 255)))
    for _ in range(3):
        x0, y0 = rnd.randint(0, w // 2), rnd.randint(0, h // 2)
        img.paste((rnd.randint(0, 255), rnd.randint(0, 255), rnd.randint(0, 255)), (x0, y0, x0 + w // 3, y0 + h // 3))
    ext = "png" if i % 3 == 0 else "jpg"
    img.save(os.path.join(sub, f"pic_{i:05d}.{ext}"), quality=85)
first = next(os.path.join(dp, f) for dp, _, fs in os.walk(out) for f in sorted(fs) if f.endswith(".png"))
shutil.copy2(first, os.path.join(out, "duplicate_of_a_picture.png"))
open(os.path.join(out, "broken.jpg"), "wb").write(b"not really a jpeg")
open(os.path.join(out, "readme.txt"), "w").write("not a picture")
os.makedirs(os.path.join(out, "skipme"))
Image.new("RGB", (640, 360), (1, 2, 3)).save(os.path.join(out, "skipme", "excluded.png"))
print("created", n, "pictures in", out)
