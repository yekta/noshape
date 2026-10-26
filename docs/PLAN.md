# How we work from here: a release every day

Written 2026-10-26 by the lead, after the user stopped all agents. This replaces the merge queue (10-22 to 10-25) and
the trunk rules (10-25 18:50 to 10-26). Read this first; then docs/HANDOFF.md and the per-area docs/handoff/*.md.

## 1. What went wrong (facts, not excuses)

- **No release in two days.** v0.4.8 was cut 10-24 23:22. Since then about 260 commits landed, but main was green in only
  6 of 35 full test runs, and the release rule (full suite green on Linux, macOS and Windows at once) never lined up.
- **Everything waited on everything.** 10 agents plus the lead's watcher shared one 16-core machine (load 25 to 40). Builds
  queued on cargo's target-dir lock: three or four agents per dir, 20 to 50 minutes in line. Testing only the model crate
  took 15 to 20 minutes; a full run took about 50; a CI run about an hour per platform. Agents spent most of their time
  "monitoring" (waiting on builds), not coding.
- **One gate for everything.** All 1,520 tests counted the same, so main went red for reasons that never mattered to a
  user, and every red blocked everyone. The reds of 10-26 came from four kinds of tests:
  1. *Cross-area behaviour changes.* One agent changed behaviour that another agent's test asserted: mate value signs,
     hidden seam edges, sketch replies without reference-only entities, edge orientation after an extrude change.
  2. *Exact-value geometry tests.* They compared against OCCT's or Parasolid's last digits, so they broke on a more exact
     integrator, a patched OCCT, GCC 13 vs 15, or macOS arm64.
  3. *Test isolation bugs.* Two tests shared one scratch file; parallel tests raced in the Vulkan loader.
  4. *"Known difference" guard tests* (oracle_p1, oracle_asm). They fail whenever a case starts or stops matching, so
     they go red on progress too.
- **Platform failures showed up hours late.** The watcher only tested Linux on this machine. macOS and Windows broke
  silently, and for one night every CI run was cancelled by the next push.
- **The lead routed instead of fixing,** let the agent count creep back up after promising fewer, and reacted to
  status flags instead of reading what agents actually did.

## 2. The plan

### 2.1 Tests in three tiers (the fix for "a million failing CIs")

| Tier | What | Time budget | Runs | Blocks |
|---|---|---|---|---|
| **T0 smoke** | builds the app and CLI; opens the sample documents and regenerates them; ~20 key e2e ops | **≤ 3 min** | before every push (all agents), every main commit | the push |
| **T1 core** | unit tests and fast integration tests: deterministic, platform-independent, no exact last-digit asserts | **≤ 10 min** on an idle machine | every main commit (watcher), CI on all 3 platforms for release candidates | main, releases |
| **T2 deep** | oracles vs live (oracle_p1, oracle_asm, std_oracle), kernel_robustness, perf/bench, big models (pattern_fins, pattern_perf, bracket_perf, many_parts), long drawing and export round trips | no limit | nightly on Linux; weekly on macOS and Windows; on demand | **nothing**: each failure becomes a ticket for its owner |

How:
- T2 tests live in test files or functions marked `#[ignore = "T2"]`. They run with `cargo test -- --ignored`. Moving
  a test between tiers is a one-line change, recorded in docs/TESTS.md together with the reason.
- **A T1 test may not:** compare geometry to more than live's tolerance band (use `[min, max]` bands from live's
  mass properties), depend on thread or test order, share files (`scratch()` already enforces unique names), or touch
  the GPU without `gpu_lock()`.
- **Known-difference lists** (KNOWN in the oracles) are T2 only. Progress never turns T1 red.
- **First task of the new thread:** measure every test file, put the slow and fragile ones in T2, and bring T1 under
  10 minutes. Candidates for T2 by size: ops/e2e.rs is split in two (core ops in T1, the rest in T2), plus model
  features_a/b/c's long cases, custom_features, kernel_robustness, std_oracle, the oracle files, sheet metal, the
  pattern and perf files, and kernel export round trips.

### 2.2 A release every day (a train, not a gate)

- **Every day at 05:00 UTC**, cut `release/vX.Y.Z` from the newest main commit that passed T0+T1 on Linux (tag `last-green`).
- **CI on that branch:** T0+T1 on Linux, macOS and Windows, plus the bundles. A platform failure is fixed **on the release
  branch**, by a cherry-picked fix or by reverting the offending commit there only. It never stops the release; main
  gets the real fix the same day.
- **Release by 09:00 UTC.** If a candidate can't be fixed by then, ship the previous day's candidate plus its fixes. A
  day without a release is the exception, and gets explained.
- **Releases never carry a known T0/T1 failure.** T2 failures are listed in the release notes as known issues.

### 2.3 Few agents, each with its own build directory

- **At most 4 building agents at once,** each in its own worktree with its **own** target dir (no shared cargo lock,
  so no queueing). About 30 GB each fits on the disk. The lead doesn't count; the live-capture agent doesn't build.
- **Areas by code, so two agents rarely edit the same file:**
  1. **Core modelling:** features, regen, kernel, FeatureScript/std, the Onshape oracles (P0/P1 results and speed).
  2. **Part Studio UI:** dialogs, feature list, viewport look and interaction, icons, selection, measure, section view.
  3. **Sketch + assembly:** sketch tools, solver, mates, assembly UI, standard content.
  4. **Drawings + agent API + data exchange** (P2), or, when P1 needs it, a second core-modelling agent.
- **Performance work** is part of each area's acceptance (budgets below), not a separate agent fighting for CPU.
  perf.yml runs the benchmarks on GitHub's macOS runners, not on this machine.
- **QA** (the workflow passes against live) is done by the area agents at the end of each day, and by the lead on the
  daily release build. No standing QA agent.

### 2.4 Landing on main

- **Before pushing:** rebase on main, then run T0 plus the T1 tests of your crates and every crate that depends on them.
  With 4 agents and separate target dirs that takes minutes, not an hour.
- **If you change behaviour that another test asserts** (sign conventions, which entities are listed, edge
  orientation, default values), update every affected test in the same commit. `git grep` for them; the reviewer is you.
- **Main red on T0/T1:** the lead reverts the offending commit **at once**; no 20-minute grace. The author re-lands it
  fixed. A revert is normal, not a punishment.
- **C++:** direct-initialise OCCT handle conversions (GCC 13 on CI rejects copy-initialisation that GCC 15 here
  accepts). The lead's watcher builds with GCC 13 like CI.

### 2.5 The lead's job

- **Release manager:** cut, watch and fix the daily train. Own the watcher and T1 health.
- **Triage:** turn T2 failures and QA findings into tickets for the right area (docs/TICKETS.md), ordered by priority
  (/root/wt/PRIORITIES.md: P0 crashes/wrong results/speed, P1 core Part Studio, P2 assembly/drawing/data exchange, P3
  rest).
- **Check agents by evidence:** their transcripts and commits (tools/lead/peek.py), not status flags. Fix blockers
  directly.
- **Never more than 4 building agents.** If someone suggests a fifth, the answer is no.

### 2.6 Performance budgets, checked in perf.yml on macOS 14/15

| Action | Budget |
|---|---|
| Edit commit on the 30-feature part | ≤ 200 ms |
| Undo / redo | ≤ 50 ms |
| Orbit / pan / zoom on big models at Retina | 60 fps |
| Assembly drag, 200 instances | 60 fps |
| Cold start to first frame | ≤ 1 s (second launch), ≤ 2 s (first launch) |
| Sketch drag step | under one 60 Hz frame |
| Feature dialog add (std editing logic) | ≤ 150 ms |

## 3. Day one for the new thread

1. **Read** this file, docs/HANDOFF.md, docs/handoff/*.md (every agent wrote its state, open items and WIP branch on
   10-26), and docs/TICKETS.md once it exists.
2. **Lead, before starting any agent:**
   - split the tests into tiers (2.1) and measure T1;
   - get main green on T0+T1;
   - cut v0.4.9 through the release train (2.2);
   - set up the watcher from tools/lead/ (it now tests T0+T1 only, plus a nightly T2 run).
3. **Start the 4 area agents** (2.3), each with: its area, its handoff doc, its WIP branch (agent/<name> on GitHub, if
   any), its own target dir, and this file.
4. **From then on,** a release every morning.

## 4. Where things are

- Lead scripts (watcher, disk loop, release cut, transcript peek, menu-coverage check): tools/lead/ (copied 10-26 from
  /tmp/lead and /root/.config/noshape/lead).
- Agent WIP branches on GitHub: `agent/<name>` (api, fs, icons2, kern, psb, qa5, skb, ui, ui2, perf), pushed 10-26.
- **The repo is /root/projects/noshape** (Motile's project; its local `main` is kept equal to origin/main by
  tools/lead/loops.sh, never committed on). /root/noshape and /root/wt/* are the retired pre-10-26 clone and its worktrees.
- Worktrees (of /root/projects/noshape): /root/projects/noshape-wt/<name>: `watch` (the watcher), `release` (release
  cuts), and one per area agent: `core`, `psui`, `skasm`, `dx`. Target dirs: /root/target-<same name>, plus
  /root/target-lead for the watcher.
- Patched OCCT: /root/occt (the unpatched build is kept in /root/occt-unpatched); patches are in patches/occt/.
- Live Onshape capture: Chrome on DISPLAY=:107 (CDP 9333); helpers in /root/live-pw/; findings in
  /root/onshape-research/live/behaviours.md.
