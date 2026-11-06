#!/usr/bin/env python3
"""Export fidelity of drawings: ours against live's export of the same sheet (lead 11-06).

    tools/drawing-export/compare.py <live dir> <ours dir> [name ...]

Each dir holds <name>.pdf / .dxf / .dwg: live's from its runner (a scenario's "export" key, after the last step),
ours from tools/ui-parity/run.py (the same key). Compared:
- DXF: per layer, each entity type's count and drawn length (lines, arcs, circles, polylines; a spline by its control
  polygon), the linetypes and lineweights used, and the texts (TEXT / MTEXT strings, formatting codes stripped).
- PDF (poppler's pdftotext -bbox and pdftocairo -svg): the page size, the words, and the stroked path length per line
  width and dash pattern.
- DWG: the format version (its header); the geometry through its DXF twin.
Lengths are equal within 2 %, counts and texts exactly. Prints one line per file and a score.
"""
import math
import re
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path


# ---------------------------------------------------------------- DXF

def dxf_pairs(text):
    lines = text.splitlines()
    for i in range(0, len(lines) - 1, 2):
        yield lines[i].strip(), lines[i + 1].rstrip("\r")


def dxf_entities(path):
    """(section, block, entity) with each entity a dict of its group codes (lists)."""
    out, section, block, cur = [], None, None, None
    for code, value in dxf_pairs(Path(path).read_text(errors="replace")):
        if code == "0":
            if cur is not None:
                out.append((section, block, cur))
                cur = None
            if value == "SECTION":
                section = "?"
            elif value in ("ENDSEC", "EOF"):
                section = None
            elif value == "BLOCK":
                block = "?"
            elif value == "ENDBLK":
                block = None
            elif section in ("ENTITIES", "BLOCKS"):
                cur = {"type": value}
            continue
        if section == "?" and code == "2":
            section = value
            continue
        if section == "BLOCKS" and block == "?" and code == "2":
            block = value
        if cur is not None:
            cur.setdefault(code, []).append(value)
    if cur is not None:
        out.append((section, block, cur))
    return out


def num(e, code, k=0, default=0.0):
    try:
        return float(e[code][k])
    except (KeyError, IndexError, ValueError):
        return default


def entity_length(e):
    t = e["type"]
    if t == "LINE":
        return math.dist((num(e, "10"), num(e, "20")), (num(e, "11"), num(e, "21")))
    if t == "CIRCLE":
        return 2 * math.pi * num(e, "40")
    if t == "ARC":
        a0, a1 = num(e, "50"), num(e, "51")
        sweep = (a1 - a0) % 360 or 360
        return math.radians(sweep) * num(e, "40")
    if t == "ELLIPSE":
        a = math.hypot(num(e, "11"), num(e, "21"))
        b = a * num(e, "40", default=1.0)
        t0, t1 = num(e, "41"), num(e, "42", default=2 * math.pi)
        sweep = (t1 - t0) % (2 * math.pi) or 2 * math.pi
        steps = 64
        pts = [(a * math.cos(t0 + sweep * k / steps), b * math.sin(t0 + sweep * k / steps)) for k in range(steps + 1)]
        return sum(math.dist(p, q) for p, q in zip(pts, pts[1:]))
    if t in ("LWPOLYLINE", "SPLINE"):
        xs, ys = e.get("10", []), e.get("20", [])
        pts = [(float(x), float(y)) for x, y in zip(xs, ys)]
        d = sum(math.dist(a, b) for a, b in zip(pts, pts[1:]))
        if t == "LWPOLYLINE" and int(num(e, "70")) & 1 and len(pts) > 2:
            d += math.dist(pts[-1], pts[0])
        return d
    return 0.0


MTEXT_CODES = re.compile(r"\\[A-Za-z][^;\\]*;|\\[PpNn~]|[{}]")


def text_of(e):
    s = "".join(e.get("3", [])) + "".join(e.get("1", []))
    return " ".join(MTEXT_CODES.sub(" ", s).split())


def dxf_summary(path):
    ents = dxf_entities(path)
    used_blocks = Counter(v for sec, blk, e in ents if sec == "ENTITIES" and e["type"] == "INSERT" for v in e.get("2", []))
    per = defaultdict(lambda: [0, 0.0])
    styles, texts = Counter(), Counter()
    for sec, blk, e in ents:
        times = 1 if sec == "ENTITIES" else used_blocks.get(blk, 0)
        if times == 0 or e["type"] in ("INSERT", "ATTDEF", "SEQEND", "VERTEX"):
            continue
        layer = e.get("8", ["0"])[0]
        if e["type"] in ("TEXT", "MTEXT", "ATTRIB"):
            t = text_of(e)
            if t:
                texts[(layer, t)] += times
            continue
        k = (layer, e["type"])
        per[k][0] += times
        per[k][1] += times * entity_length(e)
        styles[(layer, e.get("6", ["BYLAYER"])[0], e.get("370", ["-1"])[0])] += times
    return {"entities": dict(per), "styles": styles, "texts": texts}


def compare_dxf(live, ours):
    a, b = dxf_summary(live), dxf_summary(ours)
    diffs = []
    for k in sorted(set(a["entities"]) | set(b["entities"])):
        (na, la), (nb, lb) = a["entities"].get(k, [0, 0.0]), b["entities"].get(k, [0, 0.0])
        if na != nb or abs(la - lb) > 0.02 * max(la, lb, 1e-9):
            diffs.append(f"{k[0]}/{k[1]}: {nb} drawn {lb:.2f}, live {na} drawn {la:.2f}")
    for k in sorted(set(a["styles"]) | set(b["styles"])):
        if (k in a["styles"]) != (k in b["styles"]):
            diffs.append(f"style layer {k[0]} linetype {k[1]} lineweight {k[2]}: {'live only' if k in a['styles'] else 'ours only'}")
    for t in sorted(set(a["texts"]) | set(b["texts"])):
        if a["texts"][t] != b["texts"][t]:
            diffs.append(f"text {t[1]!r} on {t[0]}: {b['texts'][t]}, live {a['texts'][t]}")
    return diffs


# ---------------------------------------------------------------- PDF

def pdf_words(path):
    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / "w.html"
        subprocess.run(["pdftotext", "-bbox", str(path), str(out)], check=True, capture_output=True)
        html = out.read_text(errors="replace")
    page = re.search(r'<page width="([\d.]+)" height="([\d.]+)"', html)
    words = Counter(w.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">") for w in re.findall(r"<word[^>]*>([^<]*)</word>", html))
    return (round(float(page.group(1)), 1), round(float(page.group(2)), 1)) if page else None, words


PATH_TOKEN = re.compile(r"[MLCZHVmlczhv]|-?\d*\.?\d+(?:e-?\d+)?")


def path_length(d):
    toks, i, pos, start, total = PATH_TOKEN.findall(d), 0, (0.0, 0.0), (0.0, 0.0), 0.0
    cmd = None
    while i < len(toks):
        t = toks[i]
        if t.isalpha():
            cmd = t
            i += 1
            if cmd in "Zz":
                total += math.dist(pos, start)
                pos = start
            continue
        if cmd in ("M", "L"):
            p = (float(toks[i]), float(toks[i + 1]))
            if cmd == "L":
                total += math.dist(pos, p)
            else:
                start = p
            pos, i = p, i + 2
        elif cmd == "C":
            pts = [(float(toks[i + k]), float(toks[i + k + 1])) for k in (0, 2, 4)]
            # A cubic's length: between its chord and its control polygon.
            total += (math.dist(pos, pts[0]) + math.dist(pts[0], pts[1]) + math.dist(pts[1], pts[2]) + math.dist(pos, pts[2])) / 2
            pos, i = pts[2], i + 6
        else:
            i += 1
    return total


def pdf_strokes(path):
    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / "p.svg"
        subprocess.run(["pdftocairo", "-svg", str(path), str(out)], check=True, capture_output=True)
        svg = out.read_text(errors="replace")
    per = Counter()
    for m in re.finditer(r"<path ([^>]*)/>", svg):
        attrs = m.group(1)
        if 'stroke="none"' in attrs or "stroke=" not in attrs:
            continue
        w = re.search(r'stroke-width="([\d.]+)"', attrs)
        dash = re.search(r'stroke-dasharray="([^"]+)"', attrs)
        d = re.search(r' d="([^"]+)"', attrs)
        if not d:
            continue
        key = (round(float(w.group(1)), 2) if w else 1.0, dash.group(1).replace(" ", "") if dash else "solid")
        per[key] += path_length(d.group(1))
    return per


def compare_pdf(live, ours):
    (pa, wa), (pb, wb) = pdf_words(live), pdf_words(ours)
    diffs = []
    if pa != pb:
        diffs.append(f"page {pb}, live {pa}")
    for w in sorted(set(wa) | set(wb)):
        if wa[w] != wb[w]:
            diffs.append(f"word {w!r}: {wb[w]}, live {wa[w]}")
    sa, sb = pdf_strokes(live), pdf_strokes(ours)
    for k in sorted(set(sa) | set(sb)):
        if abs(sa[k] - sb[k]) > 0.02 * max(sa[k], sb[k], 1e-9):
            diffs.append(f"strokes width {k[0]} dash {k[1]}: {sb[k]:.1f} pt, live {sa[k]:.1f}")
    return diffs


# ---------------------------------------------------------------- DWG

def compare_dwg(live, ours):
    va, vb = Path(live).read_bytes()[:6].decode(errors="replace"), Path(ours).read_bytes()[:6].decode(errors="replace")
    return [] if va == vb else [f"version {vb}, live {va}"]


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    live, ours = Path(sys.argv[1]), Path(sys.argv[2])
    names = sys.argv[3:] or sorted({p.stem for p in live.iterdir() if p.suffix in (".pdf", ".dxf", ".dwg")})
    ok = total = 0
    for name in names:
        for ext, cmp in ((".pdf", compare_pdf), (".dxf", compare_dxf), (".dwg", compare_dwg)):
            a, b = live / f"{name}{ext}", ours / f"{name}{ext}"
            if not a.is_file():
                continue
            total += 1
            if not b.is_file():
                print(f"{name}{ext}: ours not exported")
                continue
            diffs = cmp(a, b)
            ok += not diffs
            print(f"{name}{ext}: " + ("matches" if not diffs else f"{len(diffs)} differences: " + "; ".join(diffs[:12])))
    print(f"export score: {ok}/{total} files match")


if __name__ == "__main__":
    raise SystemExit(main())
