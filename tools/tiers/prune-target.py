import os, re, sys, time, shutil, collections
# Keep the newest 2 builds of each noshape crate's library and of each of its test/bin executables per target dir (old ones
# pile up when features or the dependency graph change). Older builds nobody wrote or read for 60 min go, with their
# fingerprint dir (same hash), so cargo rebuilds instead of failing on a missing file. --dry: report only. Skips target dirs
# a cargo is using. Used by land-remote.sh on the land server (and the lead's disk loop / watcher).
KEEP = 2; AGE = 3600; dry = '--dry' in sys.argv; now = time.time(); freed = 0; n = 0
def busy(t):
    # Never in a dir a cargo is using: deleting a fingerprint dir mid-build fails that build (the watcher's clippy, 10-28 05:10).
    for pid in os.listdir('/proc'):
        try:
            if pid.isdigit() and open(f'/proc/{pid}/comm').read().strip() in ('cargo', 'rustc', 'clippy-driver') and \
                    f'CARGO_TARGET_DIR={t}'.encode() in open(f'/proc/{pid}/environ', 'rb').read().split(b'\0'):
                return True
        except OSError:
            pass
    return False
for t in [a for a in sys.argv[1:] if a != '--dry' and not busy(a)]:
    deps = os.path.join(t, 'debug/deps'); fp = os.path.join(t, 'debug/.fingerprint')
    pkg = {}
    for d in os.listdir(fp):
        p, _, h = d.rpartition('-')
        if p.startswith('noshape'): pkg[h] = (p, d)
    groups = collections.defaultdict(dict)
    for f in os.scandir(deps):
        m = re.fullmatch(r'(.+)-([0-9a-f]{16})(\.\w+)?', f.name)
        if not m or not f.is_file() or m.group(2) not in pkg or m.group(3) == '.d': continue
        kind = 'lib' if m.group(3) in ('.rlib', '.rmeta', '.so') else 'exe' if m.group(3) is None else None
        if kind is None: continue
        st = f.stat(); u = groups[(pkg[m.group(2)][0], kind, m.group(1))].setdefault(m.group(2), [0, 0, 0, []])
        u[0] = max(u[0], st.st_mtime); u[1] = max(u[1], st.st_atime); u[2] += st.st_size; u[3].append(f.path)
    for g in groups.values():
        for h, (mt, at, sz, files) in sorted(g.items(), key=lambda kv: -kv[1][0])[KEEP:]:
            if now - mt < AGE or now - at < AGE: continue
            freed += sz; n += 1
            if dry: continue
            for p in files:
                try: os.remove(p)
                except OSError: pass
            shutil.rmtree(os.path.join(fp, pkg[h][1]), ignore_errors=True)
print(f"prune-exes: {'would remove' if dry else 'removed'} {n} old builds of noshape crates, {freed/1e9:.1f} GB")
