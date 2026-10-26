#!/usr/bin/env python3
# Prints the names of the T2 tests (`#[ignore = "T2..."]`, docs/TESTS.md), one per line, as libtest names them:
# integration tests by their path inside the test file (top-level fns: just the name), unit tests in src by module path.
import os, re, sys
root = sys.argv[1] if len(sys.argv) > 1 else '.'
attr = re.compile(r'#\[ignore\s*=\s*"T2')
fn = re.compile(r'^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)')
mod = re.compile(r'^(\s*)(?:pub(?:\([a-z]+\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*\{')
for crate in sorted(os.listdir(os.path.join(root, 'crates'))):
    base = os.path.join(root, 'crates', crate)
    for dirpath, _, files in os.walk(base):
        for f in files:
            if not f.endswith('.rs'): continue
            p = os.path.join(dirpath, f); rel = os.path.relpath(p, base)
            lines = open(p, encoding='utf8').read().splitlines()
            if not any(attr.search(l) for l in lines): continue
            # Module path: for src files, from the file path; inline `mod x {` blocks tracked by indentation.
            if rel.startswith('tests' + os.sep):
                prefix = [] if rel.count(os.sep) == 1 else rel.split(os.sep)[1:-1]
            else:
                parts = rel.split(os.sep)[1:]
                parts[-1] = parts[-1][:-3]
                if parts[-1] in ('lib', 'main', 'mod'): parts = parts[:-1]
                prefix = parts
            stack = []; pending = False
            for l in lines:
                m = mod.match(l)
                if m: stack = [s for s in stack if s[0] < len(m.group(1))] + [(len(m.group(1)), m.group(2))]; continue
                if attr.search(l): pending = True; continue
                if pending:
                    m = fn.match(l)
                    if m:
                        ind = len(l) - len(l.lstrip())
                        mods = [n for i, n in stack if i < ind]
                        print('::'.join(prefix + mods + [m.group(1)])); pending = False
