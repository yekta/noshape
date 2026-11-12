#!/usr/bin/env python3
"""The build host's half of tools/bool/gate.sh: the Boolean corpora and parity56 replayed, the model suite and the
knurl / gear timing, each case once, in parallel within the host's cores; results cached by what they were built from.

  gate-host.py fp                                    the host's part of the cache keys (corpora, OCCT, models)
  gate-host.py cmpfiles BEFORE AFTER                 two replays' results compared (kall-nightly.sh)
  gate-host.py run --out DIR --role NAME:RKEY:SKEY:OKEY:KBIN:OBIN ... [--speed BASE_KBIN NEW_KBIN] --cmp BASE_ROLE NEW_ROLE

A role's replay (kernel binary KBIN) and suite (ops binary OBIN, '-' for none) run only when the cache has no result
for its key: ~/agents/bool-gate/cache/<RKEY>-replay.tsv, <SKEY>-suite.tsv. OCCT's results, which only change with
what OCCT's path is built from (OKEY), are kept by case in ~/agents/bool-gate/occt/<OKEY> (NOSHAPE_BREP_OCCT_CACHE). Run on the host (gate.sh does it through rb.sh).
"""
import argparse, hashlib, json, os, re, shutil, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor

HOME = os.path.expanduser("~")
CORPORA = f"{HOME}/agents/bool-corpora"
MODELS = f"{HOME}/parity/models"
CACHE = f"{HOME}/agents/bool-gate/cache"
# Bumped when what a result file holds or how a log is read changes: older cached results are not reused.
FORMAT = "1"
# The gate's corpora: bool's whyall set, shfall, the knurls and gears (kg.sh). parity56 against OCCT run here. kall
# (knurls of 45-50 GB a process) is the nightly run's (kall-nightly.sh, lead 11-12), not each change's.
REPLAY = ("bool-corpus corpus2 corpus3 corpus4 corpus5 corpus6 direct-corpus sh-new2 parity56 valve2 snap sp-cut shfall "
          "k10-25 k10-25-12 k25-15 k25-25 k25-71-12 cf-spur-gear gears gpf").split()
VS_OCCT = {"parity56"}
# Corpora whose cases take many GB (the memory budget is 16 GB a process, as whyall.sh's): Shell's slab fuses and the
# gears in a pool of two; the knurls, whose OCCT cuts reached 35-40 GB a process, one at a time, each with 36 GB free
# (vibe4's earlyoom ends the largest process when memory runs low: eight heavy shards at once lost their results);
# a case a shard (kall's 324c79d1 alone took 49.5 GB).
# A shard ended without its results runs again alone after the rest.
HEAVY = {"shfall", "cf-spur-gear", "gears", "gpf"}
SOLO = {"kall", "k10-25", "k10-25-12", "k25-15", "k25-25", "k25-71-12"}
HEAVY_AT_ONCE = 2
# The cases timed, ours alone, main's binary and the branch's alternating.
SPEED = ["k25-15", "k25-25", "cf-spur-gear"]
# (A case is named by its file: a recorded one by its 16-digit hash, a hand-made one (kall's k25x2) any way.)
NAME = r"[\w.-]+"
CASE = re.compile(rf"^({NAME})( \(|: failed)")


def sha(*parts):
    h = hashlib.sha1()
    for p in parts:
        h.update(p.encode() if isinstance(p, str) else p)
        h.update(b"\0")
    return h.hexdigest()


def listing(root, recursive=False):
    """Names, sizes and mtimes under root: changes when any file does."""
    rows = []
    for d, dirs, files in os.walk(root, followlinks=True):
        dirs.sort()
        for f in sorted(files):
            try:
                s = os.stat(os.path.join(d, f))
            except OSError:
                continue
            rows.append(f"{os.path.relpath(os.path.join(d, f), root)} {s.st_size} {int(s.st_mtime)}")
        if not recursive:
            break
    return "\n".join(rows)


def fingerprints():
    occt = listing("/opt/occt/lib") + listing("/opt/opennurbs/lib")
    corpora = "".join(f"{c}\n{listing(os.path.join(CORPORA, c))}" for c in REPLAY)
    return {"replay": sha(FORMAT, occt, corpora), "suite": sha(FORMAT, occt, listing(MODELS, True))}


# ---- the replay ----------------------------------------------------------------------------------------------------

def read_lines(path):
    try:
        return open(path).read().splitlines()
    except OSError:
        return []


def corpus_cases():
    """Every distinct case: {key: case}, a case being the same inputs (.bin / .brep), op, options and expected result
    (its .txt but the recorded time) replayed the same way, in whichever corpora hold it."""
    cases = {}
    for c in REPLAY:
        d = os.path.join(CORPORA, c)
        if not os.path.isdir(d):
            continue
        unstable = {l.strip() for l in read_lines(os.path.join(d, "unstable"))}
        live = {l.split(" ", 1)[0]: l for l in (l.strip() for l in read_lines(os.path.join(d, "live"))) if " " in l}
        for f in sorted(os.listdir(d)):
            if not f.endswith(".txt") or f[:-4] in unstable:
                continue
            name = f[:-4]
            txt = read_lines(os.path.join(d, f))
            ms = next((float(l.split()[1]) for l in txt if l.startswith("ms ")), 0.0)
            inputs = [x for x in (f"{name}.bin", f"{name}.brep") if os.path.exists(os.path.join(d, x))]
            body = open(os.path.join(d, inputs[0]), "rb").read() if inputs else b""
            key = sha(name, "\n".join(l for l in txt if not l.startswith("ms ")), hashlib.sha1(body).hexdigest(),
                      "vs_occt" if c in VS_OCCT else "", live.get(name, ""))[:20]
            x = cases.setdefault(key, {"key": key, "name": name, "dir": d, "files": [f] + inputs, "ms": ms,
                                       "vs_occt": c in VS_OCCT, "live": live.get(name), "corpora": []})
            x["corpora"].append(c)
    return cases


def replay_tasks(role, out, kbin, jobs, okey):
    """Shards (directories of links to the cases' files) balanced by the recorded times, longest first. A shard
    holds one case of a name and one kind (parity56's against OCCT run here, or not; its memory class)."""
    cases = corpus_cases()
    pools = {}
    for x in sorted(cases.values(), key=lambda x: (x["name"], x["key"])):
        heavy = "solo" if any(c in SOLO for c in x["corpora"]) else "heavy" if any(c in HEAVY for c in x["corpora"]) else ""
        variant = 0
        while (x["vs_occt"], heavy, variant, x["name"]) in pools.setdefault("_seen", set()):
            variant += 1
        pools["_seen"].add((x["vs_occt"], heavy, variant, x["name"]))
        pools.setdefault((x["vs_occt"], heavy, variant), []).append(x)
    pools.pop("_seen")
    total = sum(x["ms"] + 30 for x in cases.values())
    tasks = []
    for (vs, heavy, variant), xs in sorted(pools.items()):
        weight = sum(x["ms"] + 30 for x in xs)
        # (By time, and by count: cases recorded with our Boolean on (parity56's) say little of the replay's time.)
        k = max(1, round(4 * jobs * weight / total), -(-len(xs) // (1 if heavy == "solo" else 25 if heavy else 40)))
        bins = [[0.0, []] for _ in range(k)]
        for x in sorted(xs, key=lambda x: -x["ms"]):
            b = min(bins, key=lambda b: b[0])
            b[0] += x["ms"] + 30
            b[1].append(x)
        for i, (w, members) in enumerate(bins):
            if not members:
                continue
            sd = os.path.join(out, role, "shards", f"{'v' if vs else heavy[:1] if heavy else 'r'}{variant}-{i}")
            os.makedirs(sd, exist_ok=True)
            for x in members:
                for f in x["files"]:
                    os.symlink(os.path.join(x["dir"], f), os.path.join(sd, f))
            live = [x["live"] for x in members if x["live"]]
            if live:
                open(os.path.join(sd, "live"), "w").write("\n".join(live) + "\n")
            occt = f"{HOME}/agents/bool-gate/occt/{okey}-{FORMAT}"
            os.makedirs(occt, exist_ok=True)
            env = {"NOSHAPE_BOOL_CORPUS": sd, "NOSHAPE_BREP_WHY": "", "NOSHAPE_BOOLEAN_MEMORY_MB": "16000", "NOSHAPE_BREP_OCCT_CACHE": occt}
            if vs:
                env["NOSHAPE_BREP_VS_OCCT"] = "1"
            tasks.append({"role": role, "kind": "replay", "id": os.path.basename(sd), "weight": w, "env": env, "heavy": heavy,
                          "cmd": [kbin, "brep_replay::brep_corpus_replay", "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                          "cases": {x["name"]: x["key"] for x in members}})
    return tasks, cases


def parse_replay(log):
    """{case name: (status, detail, ms)} from one replay's output."""
    got = {}
    lines = read_lines(log)
    differ = False
    for l in lines:
        m = re.match(rf"^({NAME}) declined: (.*)$", l)
        if m:
            got[m[1]] = ("declined", m[2], "")
            continue
        m = re.match(rf"^({NAME}) (Fuse|Cut|Common) faces \d+ tools \d+: ([\d.]+) ms", l)
        if m:
            got.setdefault(m[1], ("same", "", m[3]))
            continue
        m = re.match(rf"^  (equivalent|as live, OCCT differs): ({NAME})", l)
        if m:
            got[m[2]] = ("equivalent" if m[1] == "equivalent" else "as-live", l.split(": ", 2)[-1][:300], got.get(m[2], ("", "", ""))[2])
            continue
        if re.match(r"^\d+ of \d+ cases differ:$", l):
            differ = True
            continue
        if differ:
            m = CASE.match(l)
            if m:
                rest = l[len(m[1]):]
                got[m[1]] = ("failed" if rest.startswith(": failed") else "differ", rest.lstrip(": ")[:400], "")
            elif not l.startswith("    "):
                differ = False
    return got


# ---- the model suite -----------------------------------------------------------------------------------------------

def documents():
    return sorted(d for d in os.listdir(MODELS) if os.path.isfile(os.path.join(MODELS, d, "features.json")))


def suite_tasks(role, out, obin, jobs):
    """The suite's documents in shards balanced by their last measured times (NOSHAPE_PARITY_ONLY lists); each
    shard's report.json lands in its own directory (NOSHAPE_PARITY_MODELS a link to the models)."""
    times = {}
    try:
        times = json.load(open(f"{HOME}/agents/bool-gate/suite-times.json"))
    except (OSError, ValueError):
        pass
    docs = documents()
    k = min(len(docs), 2 * jobs)
    bins = [[0.0, []] for _ in range(k)]
    for d in sorted(docs, key=lambda d: -times.get(d, 20000.0)):
        b = min(bins, key=lambda b: b[0])
        b[0] += times.get(d, 20000.0)
        b[1].append(d)
    tasks = []
    for i, (w, members) in enumerate(bins):
        sd = os.path.join(out, role, "suite", f"s{i}")
        os.makedirs(sd, exist_ok=True)
        os.symlink(MODELS, os.path.join(sd, "models"))
        tasks.append({"role": role, "kind": "suite", "id": f"s{i}", "weight": w,
                      "env": {"NOSHAPE_PARITY_MODELS": os.path.join(sd, "models"), "NOSHAPE_PARITY_ONLY": ",".join(members)},
                      "cmd": [obin, "parity::parity_models", "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                      "docs": members})
    return tasks


def parse_suite(log, docs):
    """{document: (outcome text without times, our regeneration ms)}: its line and the indented lines under it."""
    blocks, cur = {}, None
    for l in read_lines(log):
        # (The test's own "test ... ... " starts the first document's line; a custom feature's id is the run's.)
        l = re.sub(r"custom:[0-9a-f]{16}:", "custom:-:", re.sub(r"^test \S+ \.\.\. ", "", l))
        m = re.match(r"^(\S+?): (.*)$", l)
        if m and m[1] in docs:
            cur = m[1]
            blocks.setdefault(cur, []).append(m[2])
        elif cur and l.startswith("  "):
            # (A one-document run's feature times (NOSHAPE_PARITY_ONLY): not its outcome.)
            if not re.search(r": \d+ ms, live (\d+ ms|-)$", l):
                blocks[cur].append(l.strip())
        else:
            cur = None
    got = {}
    for d, b in blocks.items():
        ms = ""
        text = []
        for l in b:
            m = re.search(r"; (reopened and )?regenerated in (\d+) ms", l)
            if m:
                ms = m[2]
                l = l[: m.start()]
            text.append(l)
        got[d] = (" | ".join(text), ms)
    return got


# ---- running -------------------------------------------------------------------------------------------------------

def total_gb():
    for l in read_lines("/proc/meminfo"):
        if l.startswith("MemTotal:"):
            return int(l.split()[1]) / 1e6
    return 1e9


def available_gb():
    for l in read_lines("/proc/meminfo"):
        if l.startswith("MemAvailable:"):
            return int(l.split()[1]) / 1e6
    return 1e9


def run_task(t, out, threads):
    """Runs one shard once the host has memory free for it (a knurl's 36 GB, another heavy one 16 GB, its budget,
    another 4 GB; on a smaller host 30 % of it at most), waiting at most 10 minutes; its peak memory and exit kept."""
    log = os.path.join(out, t["role"], "logs", f"{t['kind']}-{t['id']}.log")
    tmp = os.path.join(out, t["role"], "tmp", f"{t['kind']}-{t['id']}")
    os.makedirs(os.path.dirname(log), exist_ok=True)
    os.makedirs(tmp, exist_ok=True)
    env = dict(os.environ, RAYON_NUM_THREADS=str(threads), TMPDIR=tmp, **t["env"])
    # (On a smaller host (vibe5: 29 GB) a share of it; never more than 10 minutes' wait.)
    need = min({"solo": 36, "heavy": 16}.get(t.get("heavy") or "", 4), 0.3 * total_gb())
    waited = time.time()
    while available_gb() < need and time.time() - waited < 600:
        time.sleep(5)
    t0 = time.time()
    with open(log, "w") as o, open(log + ".err", "w") as e:
        p = subprocess.Popen(t["cmd"], stdout=o, stderr=e, env=env)
        deadline = t0 + 5400
        while True:
            pid, status, usage = os.wait4(p.pid, os.WNOHANG)
            if pid:
                rc = os.waitstatus_to_exitcode(status)
                t["peak_gb"] = usage.ru_maxrss / 1e6
                break
            if time.time() > deadline:
                p.kill()
                rc = "timeout"
                os.wait4(p.pid, 0)
                break
            time.sleep(0.5)
        p.returncode = 0
    shutil.rmtree(tmp, ignore_errors=True)
    # (A binary without the test (the wrong one): no results, said so.)
    if rc == 0 and any(l.startswith("running 0 tests") for l in read_lines(log)):
        rc = "no such test in its binary"
    t["rc"], t["secs"], t["log"] = rc, time.time() - t0, log
    return t


def write_tsv(path, rows):
    tmp = path + ".part"
    with open(tmp, "w") as f:
        for r in rows:
            f.write("\t".join(str(x).replace("\t", " ").replace("\n", " ") for x in r) + "\n")
    os.replace(tmp, path)


def read_tsv(path):
    return [l.split("\t") for l in read_lines(path)]


def collect_replay(tasks, cases, role):
    rows = []
    for t in tasks:
        got = parse_replay(t["log"]) | parse_replay(t["log"] + ".err")
        for name, key in t["cases"].items():
            why = "killed (memory?)" if t["rc"] == -9 else f"exit {t['rc']}"
            st, detail, ms = got.get(name, ("noresult", f"no line for it ({why}, {t['log']})", ""))
            rows.append((key, name, ",".join(cases[key]["corpora"]), st, detail, ms))
    return sorted(rows)


def collect_suite(tasks):
    rows = []
    for t in tasks:
        got = parse_suite(t["log"], set(t["docs"]))
        for d in t["docs"]:
            text, ms = got.get(d, ("noresult", ""))
            if text == "noresult":
                text = f"noresult: no line for it (exit {t['rc']}, {t['log']})"
            rows.append((d, text, ms))
    return sorted(rows)


def speed(base_kbin, new_kbin, out, threads, rounds=5):
    """Our time alone (NOSHAPE_BREP_OURS_ONLY), main's binary and the branch's alternating, each case's best of
    `rounds`: {corpus: (base ms, new ms, noise)}, the noise main's own rounds' spread (its slowest round over its
    fastest, less one: the host's load, which the verdict allows for)."""
    best, totals = {}, {}
    for r in range(rounds):
        for role, kbin in ([("base", base_kbin), ("new", new_kbin)] if r % 2 == 0 else [("new", new_kbin), ("base", base_kbin)]):
            for c in SPEED:
                d = os.path.join(CORPORA, c)
                if not os.path.isdir(d):
                    continue
                t = run_task({"role": "speed", "kind": f"{role}-{c}", "id": str(r), "env": {
                    "NOSHAPE_BOOL_CORPUS": d, "NOSHAPE_BREP_OURS_ONLY": "1", "NOSHAPE_BOOLEAN_MEMORY_MB": "16000"},
                    "cmd": [kbin, "brep_replay::brep_corpus_replay", "--exact", "--ignored", "--nocapture", "--test-threads=1"]}, out, threads)
                total = 0.0
                for l in read_lines(t["log"]) + read_lines(t["log"] + ".err"):
                    m = re.match(rf"^({NAME}) (Fuse|Cut|Common) (?:faces \d+ )?tools \d+: ([\d.]+) ms", l)
                    if m:
                        k = (c, m[1], role)
                        best[k] = min(best.get(k, 1e18), float(m[3]))
                        total += float(m[3])
                totals.setdefault((c, role), []).append(total)
    res = {}
    for c in SPEED:
        names = {k[1] for k in best if k[0] == c and (c, k[1], "base") in best and (c, k[1], "new") in best}
        if names:
            runs = [x for x in totals.get((c, "base"), []) if x > 0]
            noise = max(runs) / min(runs) - 1 if len(runs) > 1 else 0.0
            res[c] = (sum(best[(c, n, "base")] for n in names), sum(best[(c, n, "new")] for n in names), noise)
    return res


# ---- the verdict ---------------------------------------------------------------------------------------------------

TAKEN = {"same", "equivalent", "as-live"}


def rounded(text, digits=9):
    """A difference's words with its numbers to `digits` digits (a volume's last digits move run to run)."""
    return re.sub(r"-?\d+\.\d+(?:e-?\d+)?", lambda m: f"{float(m[0]):.{digits}g}", text)
BAD = {"differ", "failed", "noresult"}


def compare(base_r, new_r, base_s, new_s, spd):
    """(verdict, lines): FAIL on a new difference, failure or crash, a document matching fewer features; CHECK on a
    case no longer taken, a document's outcome otherwise changed, a timing case slower beyond the noise; else PASS."""
    fail, check, info = [], [], []
    if base_r is not None and new_r is not None:
        b = {r[0]: r for r in base_r}
        n = {r[0]: r for r in new_r}
        per = {}
        new_bad, lost, gained, fixed, reasons, unknown, unrunnable = [], [], [], [], 0, [], []
        for key in sorted(set(b) | set(n)):
            rb, rn = b.get(key), n.get(key)
            if rn is None or rb is None:
                info.append(f"case {key} only in {'base' if rn is None else 'branch'}'s run")
                continue
            sb, sn = rb[3], rn[3]
            for c in rn[2].split(","):
                p = per.setdefault(c, [0, 0, 0, 0, 0])
                p[0] += 1
                p[1] += sb in TAKEN
                p[2] += sn in TAKEN
                p[3] += sb in BAD
                p[4] += sn in BAD
            if sb == sn == "noresult" and all(re.search(r"\(killed|\(exit -(9|15),", x) for x in (rb[4], rn[4])):
                unrunnable.append(f"{rn[2].split(',')[0]} {rn[1]}")
            elif sb == "noresult":
                unknown.append(f"{rn[2].split(',')[0]} {rn[1]}: {rb[4]}")
            elif sn in BAD and not (sb == sn and rounded(rb[4]) == rounded(rn[4])):
                new_bad.append(f"{rn[2].split(',')[0]} {rn[1]}: {sn}: {rn[4]} (main: {sb} {rb[4]})".strip())
            elif sb in TAKEN and sn == "declined":
                lost.append(f"{rn[2].split(',')[0]} {rn[1]}: now declined: {rn[4]}")
            elif sb == "declined" and sn in TAKEN:
                gained.append(rn[1])
            elif sb in BAD and sn not in BAD:
                fixed.append(f"{rn[1]} ({sn})")
            elif sb == sn == "declined" and rb[4] != rn[4]:
                reasons += 1
        # (Cases neither run could finish for memory (kall's knurls: 45-50 GB a process, ended by the host's earlyoom;
        # bool's kg.sh got nothing for kall either): said, not failed.)
        if unrunnable:
            check.append(f"{len(unrunnable)} cases too big for this host on both sides: {' '.join(unrunnable[:8])}")
        if unknown:
            fail.append(f"{len(unknown)} cases without main's result (its replay crashed or timed out there: run again)")
            info += ["  no main result: " + x for x in unknown[:10]]
        if new_bad:
            fail.append(f"{len(new_bad)} cases differ from OCCT or fail that didn't on main")
            info += ["  new: " + x for x in new_bad[:40]]
        if lost:
            check.append(f"{len(lost)} cases main takes now declined")
            info += ["  lost: " + x for x in lost[:40]]
        info.append(f"replay: {len(n)} distinct cases; taken {sum(r[3] in TAKEN for r in base_r)} -> {sum(r[3] in TAKEN for r in new_r)} "
                    f"(+{len(gained)} -{len(lost)}), differing {sum(r[3] in BAD for r in base_r)} -> {sum(r[3] in BAD for r in new_r)}, "
                    f"fixed {len(fixed)}, decline reasons changed {reasons}")
        # (Our time on the cases both take, both runs in one pool at once: a hint; the timing rounds decide.)
        tb = {k: float(r[5]) for k, r in b.items() if r[3] in TAKEN and r[5]}
        tn = {k: float(r[5]) for k, r in n.items() if r[3] in TAKEN and r[5]}
        for what, pick in [("all", lambda c: True), ("knurls", lambda c: c in SOLO), ("gears", lambda c: c in {"cf-spur-gear", "gears", "gpf"})]:
            ks = [k for k in tb if k in tn and any(pick(c) for c in n[k][2].split(","))]
            if ks:
                sb_, sn_ = sum(tb[k] for k in ks), sum(tn[k] for k in ks)
                info.append(f"  ours on the {len(ks)} cases both take ({what}): main {sb_ / 1e3:.1f} s, branch {sn_ / 1e3:.1f} s ({sn_ / max(sb_, 1e-9):.2f}x; same pool, a hint)")
        if fixed:
            info.append("  fixed: " + ", ".join(fixed[:20]))
        if gained:
            info.append("  gained: " + " ".join(gained[:30]) + (" ..." if len(gained) > 30 else ""))
        for c in REPLAY:
            if c in per:
                p = per[c]
                info.append(f"  {c}: {p[0]} cases, taken {p[1]} -> {p[2]}, differing {p[3]} -> {p[4]}")
    if base_s is not None and new_s is not None:
        b = {r[0]: r for r in base_s}
        n = {r[0]: r for r in new_s}
        frac = lambda t: tuple(int(x) for x in re.findall(r"^(\d+)/(\d+) features match", t)[0]) if re.match(r"^\d+/\d+ features match", t) else None
        worse, changed, better, tb, tn = [], [], [], 0.0, 0.0
        for d in sorted(set(b) | set(n)):
            tb_, tn_ = (b.get(d) or ["", "", ""]), (n.get(d) or ["", "", ""])
            if tb_[2] and tn_[2]:
                tb += float(tb_[2])
                tn += float(tn_[2])
            # (A document's measures to 7 digits: an area's 1e-8 moves with the Booleans' paths, c2-rings-loft's.)
            if rounded(tb_[1], 7) == rounded(tn_[1], 7):
                continue
            fb, fn = frac(tb_[1]), frac(tn_[1])
            line = f"{d}: main '{tb_[1][:200]}' -> '{tn_[1][:200]}'"
            if tn_[1].startswith("noresult") or (fb and fn and fn[0] < fb[0]) or (fb and not fn):
                worse.append(line)
            elif fb and fn and fn[0] > fb[0]:
                better.append(line)
            else:
                changed.append(line)
        matched = lambda rows: sum(1 for r in rows if "; matches to the end" in r[1])
        feats = lambda rows: [sum(f[i] for f in (frac(r[1]) for r in rows) if f) for i in (0, 1)]
        fb_, fn_ = feats(base_s), feats(new_s)
        info.append(f"model suite: {len(n)} documents; to the end {matched(base_s)} -> {matched(new_s)}, features {fb_[0]}/{fb_[1]} -> {fn_[0]}/{fn_[1]}; "
                    f"our regeneration total {tb / 1e3:.1f} -> {tn / 1e3:.1f} s (not alternated: a hint only)")
        if worse:
            fail.append(f"{len(worse)} documents worse")
        if changed:
            check.append(f"{len(changed)} documents' outcome changed")
        info += ["  worse: " + x for x in worse] + ["  changed: " + x for x in changed] + ["  better: " + x for x in better]
    for c, (bm, nm, noise) in (spd or {}).items():
        ratio = nm / max(bm, 1e-9)
        info.append(f"speed {c}: main {bm:.0f} ms, branch {nm:.0f} ms ({ratio:.2f}x; ours alone, best of 5 alternating; main's rounds {noise:.0%} apart)")
        if ratio > 1 + max(0.05, noise):
            check.append(f"{c} {ratio:.2f}x slower (beyond main's own {noise:.0%} round to round)")
    verdict = "FAIL" if fail else "CHECK" if check else "PASS"
    return verdict, fail + check, info


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd")
    ap.add_argument("--out")
    ap.add_argument("--role", action="append", default=[])
    ap.add_argument("--speed", nargs=2)
    ap.add_argument("--cmp", nargs=2)
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--speed-threads", type=int, default=8)
    ap.add_argument("--corpora", help="these corpora only (comma-separated), not the gate's")
    ap.add_argument("files", nargs="*")
    a = ap.parse_args()
    a.out = os.path.abspath(a.out) if a.out else a.out
    if a.corpora:
        REPLAY[:] = a.corpora.split(",")
    # (A host without the memory a knurl's OCCT cut takes (35-50 GB a process; vibe5 has 29): the knurls left out,
    # said so in the verdict.)
    small = total_gb() < 48
    if small:
        REPLAY[:] = [c for c in REPLAY if c not in SOLO]
    if a.cmd == "fp":
        print(json.dumps(fingerprints()))
        return
    # (cmpfiles BEFORE AFTER: two replays' results compared, as main's and a branch's: the nightly run's.)
    if a.cmd == "cmpfiles":
        verdict, why, info = compare(read_tsv(a.files[0]), read_tsv(a.files[1]), None, None, None)
        for l in info:
            print(l)
        print(f"VERDICT {verdict}" + (f": {'; '.join(why)}" if why else ""))
        return
    os.makedirs(CACHE, exist_ok=True)
    t0 = time.time()
    # Each role (base, new): its replay key and binary, its suite key and binary ('-': none). The same key in both
    # roles (a branch that leaves those crates alone) is run once.
    roles = {}
    for spec in a.role:
        role, rkey, skey, okey, kbin, obin = spec.split(":")
        roles[role] = {"replay": rkey, "suite": skey}
    tasks, replays, scheduled = [], {}, set()
    for spec in a.role:
        role, rkey, skey, okey, kbin, obin = spec.split(":")
        if kbin != "-" and rkey not in scheduled and not os.path.exists(f"{CACHE}/{rkey}-replay.tsv"):
            scheduled.add(rkey)
            ts, cases = replay_tasks(role, a.out, kbin, a.jobs, okey)
            tasks += ts
            replays[rkey] = (ts, cases)
        if obin != "-" and skey not in scheduled and not os.path.exists(f"{CACHE}/{skey}-suite.tsv"):
            scheduled.add(skey)
            ts = suite_tasks(role, a.out, obin, a.jobs)
            for t in ts:
                t["key"] = skey
            tasks += ts
    print(f"gate-host: {len(tasks)} shards on {a.jobs} jobs x {a.threads} threads", flush=True)
    # (The knurls one at a time, the other heavy shards HEAVY_AT_ONCE at a time, the rest beside them; longest first.)
    of = lambda kind: sorted((t for t in tasks if (t.get("heavy") or "") == kind), key=lambda t: -t["weight"])
    with ThreadPoolExecutor(1) as sp, ThreadPoolExecutor(HEAVY_AT_ONCE) as hp, ThreadPoolExecutor(max(1, a.jobs - HEAVY_AT_ONCE - 1)) as lp:
        futures = [sp.submit(run_task, t, a.out, a.threads) for t in of("solo")] + [hp.submit(run_task, t, a.out, a.threads) for t in of("heavy")]
        futures += [lp.submit(run_task, t, a.out, a.threads) for t in of("")]
        done = [f.result() for f in futures]
    # (A shard killed without its results (for memory, beside the others): again, alone.)
    for t in done:
        if isinstance(t["rc"], int) and t["rc"] not in (0, 101):
            print(f"gate-host: {t['role']} {t['kind']}-{t['id']} ended {t['rc']} without its results: again alone", flush=True)
            run_task(t, a.out, a.threads)
    results = [(key, "replay", collect_replay(ts, cases, "")) for key, (ts, cases) in replays.items()]
    for skey in {t["key"] for t in done if t["kind"] == "suite"}:
        rows = collect_suite([t for t in done if t["kind"] == "suite" and t["key"] == skey])
        results.append((skey, "suite", rows))
        try:
            times = json.load(open(f"{HOME}/agents/bool-gate/suite-times.json"))
        except (OSError, ValueError):
            times = {}
        times.update({r[0]: float(r[2]) for r in rows if r[2]})
        json.dump(times, open(f"{HOME}/agents/bool-gate/suite-times.json", "w"))
    # (Kept for the next gate only when every case and document has a result: a crash or timeout is run again.)
    for key, kind, rows in results:
        write_tsv(f"{a.out}/{key}-{kind}.tsv", rows)
        if not any(str(r[3] if kind == "replay" else r[1]).startswith("noresult") for r in rows):
            shutil.copy(f"{a.out}/{key}-{kind}.tsv", f"{CACHE}/{key}-{kind}.tsv")
    if done:
        slow = sorted(done, key=lambda t: -t["secs"])[:3]
        print(f"gate-host: shards done in {time.time() - t0:.0f} s (longest: {', '.join(f'{t['kind']}-{t['id']} {t['secs']:.0f} s' for t in slow)}; "
              f"most memory: {', '.join(f'{t['kind']}-{t['id']} {t.get('peak_gb', 0):.1f} GB' for t in sorted(done, key=lambda t: -t.get('peak_gb', 0))[:3])})", flush=True)
    spd = speed(a.speed[0], a.speed[1], a.out, a.speed_threads) if a.speed else None
    if a.cmp:
        def get(role, kind):
            key = roles.get(role, {}).get(kind, "-")
            for p in (f"{a.out}/{key}-{kind}.tsv", f"{CACHE}/{key}-{kind}.tsv"):
                if key != "-" and os.path.exists(p):
                    return read_tsv(p)
            return None
        b, n = a.cmp
        verdict, why, info = compare(get(b, "replay"), get(n, "replay"), get(b, "suite"), get(n, "suite"), spd)
        for l in info:
            print(l)
        if small:
            why.append(f"the knurls ({', '.join(sorted(SOLO))}) not run: this host has {total_gb():.0f} GB, a knurl's OCCT cut takes 35-50")
            verdict = "CHECK" if verdict == "PASS" else verdict
        print(f"VERDICT {verdict}" + (f": {'; '.join(why)}" if why else ""))

if __name__ == "__main__":
    main()
