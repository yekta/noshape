#!/usr/bin/env python3
# Show what each noshape agent is doing right now: last few tool calls/texts from its transcript tail.
import glob, os, re, json, sys, time
N = int(sys.argv[1]) if len(sys.argv) > 1 else 4
files = [f for f in glob.glob('/root/.claude*/projects/*reverse-engineer*/*.jsonl') if time.time() - os.path.getmtime(f) < 12*3600 and os.path.getsize(f) > 2_000_000]
def name_of(f):
    with open(f, 'rb') as fh:
        head = fh.read(400_000).decode('utf8', 'ignore')
    m = re.search(r'agent name is \*\*([a-z0-9]+)\*\*', head) or re.search(r'You are \*\*([a-z0-9]+)\*\*', head) or re.search(r'\(([a-z0-9]{2,7})\)\\?"? ?[,.]', head)
    return m.group(1) if m else '?'
def tail(f, nbytes=3_000_000):
    with open(f, 'rb') as fh:
        fh.seek(max(0, os.path.getsize(f) - nbytes)); data = fh.read().decode('utf8', 'ignore')
    out = []
    for line in data.splitlines()[1:]:
        try: e = json.loads(line)
        except Exception: continue
        if e.get('type') != 'assistant': continue
        ts = e.get('timestamp', '')[11:16]
        for c in e.get('message', {}).get('content', []):
            if c.get('type') == 'tool_use':
                i = c.get('input', {}); s = i.get('description') or i.get('command') or i.get('text') or i.get('file_path') or json.dumps(i)[:150]
                out.append(f"{ts} [{c['name']}] {str(s)[:150]}")
            elif c.get('type') == 'text' and c.get('text', '').strip():
                out.append(f"{ts} » {c['text'].strip()[:200]}")
    return out
for f in sorted(files, key=os.path.getmtime):
    idle = (time.time() - os.path.getmtime(f)) / 60
    flag = f"  <<< IDLE {idle:.0f} min: check agent_status (needs_approval?) or a dead wait" if idle > 20 else ""
    print(f"== {name_of(f)}  ({os.path.basename(f)[:8]}, {f.split('/')[2]}, mtime {time.strftime('%H:%M', time.gmtime(os.path.getmtime(f)))}){flag}")
    for l in tail(f)[-N:]: print('   ', l.replace('\n', ' '))
