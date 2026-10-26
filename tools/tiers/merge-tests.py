#!/usr/bin/env python3
# Puts each crate's integration test files into one test binary, tests/all.rs (`#[path] mod <file>;`), so a build links
# one big test executable per crate instead of one per file (each links all of OCCT and gpui: 100-600 MB apiece).
# Files that set process-wide env vars stay separate binaries (STANDALONE). Re-run after adding a test file; t0.sh
# fails when a tests/*.rs file is in neither all.rs nor a [[test]] entry.
import os, re, sys
STANDALONE = {('kernel', 'band_mesh'), ('kernel', 'strip_mesh'), ('kernel', 'boolean_budget'), ('ops', 'prefs')}
RENAME = {'core': 'kernel_core'}  # a module named `core` would shadow the core crate
root = sys.argv[1] if len(sys.argv) > 1 else '.'
for crate in sorted(os.listdir(os.path.join(root, 'crates'))):
    d = os.path.join(root, 'crates', crate); td = os.path.join(d, 'tests')
    if not os.path.isdir(td): continue
    stems = sorted(f[:-3] for f in os.listdir(td) if f.endswith('.rs') and f != 'all.rs')
    merged = [s for s in stems if (crate, s) not in STANDALONE]
    alone = [s for s in stems if (crate, s) in STANDALONE]
    if len(merged) < 2 and not os.path.exists(os.path.join(td, 'all.rs')): continue
    with open(os.path.join(td, 'all.rs'), 'w') as f:
        f.write('//! Every integration test file of this crate in one test binary (tools/tiers/merge-tests.py): one link\n'
                '//! instead of one per file. Run one file\'s tests with `cargo test -p <crate> --test all <file>::`.\n\n')
        for s in merged:
            m = RENAME.get(s, s)
            f.write(f'#[path = "{s}.rs"]\nmod {m};\n' if m != s else f'mod {s};\n')
    toml = os.path.join(d, 'Cargo.toml'); t = open(toml).read()
    t = re.sub(r'\n# Integration tests \(tools/tiers/merge-tests.py\).*?(?=\n\[(?!\[test\])|\Z)', '', t, flags=re.S)
    if 'autotests = false' not in t:
        t = re.sub(r'(\[package\]\n(?:[^\[\n][^\n]*\n|\n)*?)(\n*\[)', lambda m: m.group(1).rstrip('\n') + '\nautotests = false\n' + '\n' + m.group(2).lstrip('\n'), t, count=1)
    block = '\n# Integration tests (tools/tiers/merge-tests.py): tests/all.rs holds every file but the standalone ones.\n'
    block += '[[test]]\nname = "all"\npath = "tests/all.rs"\n'
    for s in alone: block += f'\n[[test]]\nname = "{s}"\npath = "tests/{s}.rs"\n'
    t = t.rstrip('\n') + '\n' + block
    open(toml, 'w').write(t)
    print(crate, len(merged), 'merged', alone)
