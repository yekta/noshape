#!/usr/bin/env python3
# Prune stale build-script dirs in <target>/debug/build: per crate name keep the newest KEEP, and never touch anything modified in the last 3 h.
import os, sys, time, shutil, collections
KEEP = 16; AGE = 3 * 3600; freed = 0
for t in sys.argv[1:]:
    b = os.path.join(t, 'debug', 'build')
    if not os.path.isdir(b): continue
    groups = collections.defaultdict(list)
    for d in os.listdir(b):
        p = os.path.join(b, d); name = d.rsplit('-', 1)[0]
        try: m = max(os.path.getmtime(p), *(os.path.getmtime(os.path.join(p, x)) for x in os.listdir(p)))
        except Exception: continue
        groups[name].append((m, p))
    for name, items in groups.items():
        items.sort(reverse=True)
        for m, p in items[KEEP:]:
            if time.time() - m < AGE: continue
            sz = sum(os.path.getsize(os.path.join(r, f)) for r, _, fs in os.walk(p) for f in fs if not os.path.islink(os.path.join(r, f)))
            shutil.rmtree(p, ignore_errors=True); freed += sz
print(f"prune-build: freed {freed/1e9:.1f} GB")
