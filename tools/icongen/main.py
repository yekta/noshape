import sys, os, importlib
sys.path.insert(0, os.path.dirname(__file__))
from base import ICONS, svg
mods = [m for m in ["part", "sketch", "constraints", "assembly", "drawing", "app", "fixes"] if os.path.exists(os.path.join(os.path.dirname(__file__), m + ".py"))]
for m in mods:
    importlib.import_module(m)
out = sys.argv[1]
os.makedirs(out, exist_ok=True)
for name, fn in ICONS.items():
    with open(os.path.join(out, name + ".svg"), "w") as fh:
        fh.write(svg(fn()))
print(len(ICONS))
