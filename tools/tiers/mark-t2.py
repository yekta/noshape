#!/usr/bin/env python3
# mark-t2.py <file.rs> "<reason>" [test names...]: marks the named tests (all #[test] fns of the file when no names
# are given) #[ignore = "T2: <reason>"]. Tests that are already #[ignore]d keep their own reason (they aren't T2).
import re, sys
path, reason, names = sys.argv[1], sys.argv[2], set(sys.argv[3:])
all_tests = not names; missing = set(names)
lines = open(path, encoding='utf8').read().split('\n')
out = []; i = 0; n = 0
while i < len(lines):
    l = lines[i]; out.append(l)
    if re.match(r'^\s*#\[test\]\s*$', l):
        j = i + 1; ignored = False
        while j < len(lines) and lines[j].strip().startswith('#['):
            ignored |= lines[j].strip().startswith('#[ignore'); j += 1
        m = re.match(r'^\s*(?:pub\s+)?fn\s+([A-Za-z0-9_]+)', lines[j]) if j < len(lines) else None
        if m and not ignored and (all_tests or m.group(1) in names):
            ind = l[:len(l) - len(l.lstrip())]
            out.append(f'{ind}#[ignore = "T2: {reason}"]'); n += 1; missing.discard(m.group(1))
    i += 1
open(path, 'w', encoding='utf8').write('\n'.join(out))
print(f'{path}: {n} marked' + (f'; NOT FOUND: {sorted(missing)}' if missing else ''))
