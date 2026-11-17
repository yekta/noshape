# bool2: current state (11-17 11:25 UTC)

Second agent on noshape's own Boolean (crates/brep, crates/kernel/cpp/rust_boolean.cpp, boolean.cpp's unify). A case
is taken only where it equals OCCT (or live, with a capture); results identical end to end (docs/DECISIONS.md 11-14).
History: docs/handoff/archive/bool2.md. Rulings: docs/DECISIONS.md (read it first).

## Unlanded work
- Worktree /root/projects/noshape-wt/bool2, branch bool2-chamfer, rebased 11-17 onto main dd344ea1f (clean):
  - 84b7ae6fc: an extrusion across a surface of revolution's axis is sectioned on noshape's own (quadric_across in
    rust_boolean.cpp: each ruling meets the profile's radius at its height; Newton in a bracket for the height).
  - dba862173: the same across a cone's axis (own_sections' cone branch sends it to quadric_across).
    Together: c2-valve-config Extrude 3's cut e651026a CPU 1.98 -> 0.90 s per 10 runs, 36/90/59 valid, volume 3.7e-12
    off OCCT's; kernel tests built with g++-13 pass (463).
  - 0e66a3355: unify_in's reordered tries integrate volumes only for a try that left fewer faces (valve's
    unify.reordered 62-142 -> 17-70 ms in Extrude 1 / 2 / Circular pattern 1, 10/10 features Ok).
  Gate60 (old main c899c2bcf): replay 3026 -> 3026, 0 differences in every corpus and parity56; FAIL only from suite
  shard s1 timing out at 5400 s on both sides (the screws' Split hang: 15 "worse" = no result either side, 3 "changed"
  = identical truncated lines). Gate61 on the rebase: unit bool2-gate61 (vibe5, from 09:22 UTC), gate61.out. Then
  hats/dock A/B, land.sh.
  Uncommitted on purpose: crates/kernel/tests/all.rs registers the scratch probes (`mod sampler; mod xprobe;`, copy in
  /root/bool2-scratch/all-with-probe.rs), crates/kernel/tests/{xprobe,sampler}.rs, scratch-docs/ (qa's docs, valve
  recording). Take all.rs out (`git checkout crates/kernel/tests/all.rs`) before land.sh, put it back after.
- OCCT 0031/0032/0033 are on main (bool landed them, the lead installed the libraries): branch bool2-oc31 obsolete,
  its worktree, vibe5 staged libraries and old A/B binaries removed 11-17.

## Running (results land in /root/bool2-scratch)
- bool2-chamfer (84b7ae6fc, dba862173, 0e66a3355) queued with land.sh 11-17 (gate61 clean: replay 3025 -> 3025,
  differing 6 -> 6, suite 86 -> 86 / 1225 -> 1225, sprockets only an untranslated note's order; hats8/15/15b + dock
  14 tabs SAME against main dd344ea1f).
- bool2-udoc's second half: cli-cur vs cli-c26 (hats/dock for bool2-c26), udoc.out.
- bool2-gate62: bool2-c26 f67b3d34d vs main dd344ea1f (gate62.out).

## Chamfer 26's cut: branch bool2-c26 (f67b3d34d, on top of bool2-chamfer), gate after gate61
- Case 297300ec45997cfc (recorded 11-17 on main; OCCT 360 ms; T1 bool_patterns::a_run_on_chamfers_end_cap_beside_a_
  near_tangent_strip, data tests/data/booleans). Cause: our section of the strip (extrusion along x) with the tool's
  cylinder face 7 ended on the tool's edge 13 4 um from its end vertex 63 (6.6 um away): split_params took it as the
  edge's end, pick made a new vertex: the section dangled, the cap (0.3 mm lens 6 um off the strip) and the slivers
  couldn't be classified.
- Changes (crates/brep/src/boolean.rs): (1) pick: a section leaving one face inside the other, on an edge of the one
  within that edge's end reach of its end vertex and within the section's + vertex's tolerance of it, ends at that
  vertex (both faces ending there = the knurl's slivers, untouched); (2) ray_state: a point off a face (> 1e-3 tol)
  but within tol of it, clear of its boundary, is on it whichever way a ray leaves (+y sliver In by one ray, its
  mirror Out); (3) after the section flips settle, a piece takes a neighbour's states across an own-solid edge no
  section lies on and no other solid's piece uses (one at a time); such a piece isn't "doomed".
- Result 36/95/62 valid, volume 1.2e-10 relative off OCCT's; replay "equivalent": history 12 records apart (section
  edges' midpoints 1 um apart: our line's range ends at x 1.5026, the vertex at 1.5; the cap's arc Deleted, OCCT's
  Modified into the kept B-spline edge). Needs the end-to-end check (DECISIONS 11-15). Kernel tests 489/489 pass.

## Valve Circular pattern 1 (case rec-valve 52970b93111ac42b, ours)
- Cone pairs: the part's 60 deg ring (z axis, r 70 at z 0, two halves u 0-90 / 90-360 deg) x 8 hole cones on parallel
  axes at 71.3 mm (r 1.5 at z 3, -30 deg, 2.3 m long). 9 of 14 pairs have curves; bool's parked-cones-apart.diff
  (mine now, /root/bool2-scratch) proves none of the 5 empty ones (they reach the half's boundary angle): dropped.
  Only lever left: own parallel-axis cone x cone sections, matching OCCT's splits (bool's cyl x cyl parked on that).
- Raw cut: ours 47/143/94, OCCT 47/142/93 (OCCT merges two crossings 36 um apart into one vertex; ours keeps a 71 um
  edge). The document's part after the pattern: 46/122/72, live's exactly (vol 46868.68 vs 46868.59): ours is right.

## Other
- Impeller 38073a30d2ffbc3e (Boolean Flansch+Gehaeuse fuse): ours 20/57/40 raw, 5.913847e-5 m^3; live 19/56/39,
  5.913789e-5; OCCT 16/47/36, 5.911132e-5 (27 mm^3 off live): ours is right (told bool 11-17). 02f2f58d / 7cb79697 by
  an edge after the replay's unify, not looked at. Cases vibe5 ~/agents/bool2-keep/imp-cur.

## Agents
- Roster (lead 11-17 10:30; only these ids): psui 4973e295-9e92-463f-a13f-2ec5258bc5f9 (11-17), core
  6692ae35-758a-4442-81c8-29f67be2e21a, blend 0e6e9942-0713-46c1-ae55-132fbbb4a14b, bool 7fa8ceab-4549-499f-9b01-
  3a289c2c95b4, dx f93a1292-5bb7-406f-9a25-59767e01878e, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, asm d8d597e0-675c-
  497b-a804-544ceab99bbd, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0,
  studios 58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2 3b9b1e66-33a0-4dfe-
  9e0a-eee216e354b2.

## Open decisions (with the lead)
- Chamfer 26 builds since 897b690f; core's Delete face 2 (OCCT defeaturing) took 17-26 s after it on 11-15; on main
  11-17 it is 6.8 s (vibe5 under load; the impeller 15.6 s a regen). Asked 11-15; likely moot now.

## Next items, in order
1. Land bool2-chamfer (gate61 clean + main green).
2. Gate bool2-c26 (rebased on what lands), hats/dock A/B, the suite's end-to-end check, land.
3. Valve Extrude 3 / Circular pattern 1: own parallel-axis cone x cone sections only if OCCT's splits can be matched;
   else parked.
4. Helical Sweep 1's Boolean: 6 OCCT pairs (cylinder x the sweep's B-spline, plane x B-spline). Parked.
5. Impeller Extrude 4: OCCT's PointDepart 16 %, BRepCheck 13 % (samples).
6. Sphere x B-spline (cf-surface-pattern d6500b16, 2.2 s): Rust part 1.4 s in 2D curve sampling of refitted sections.

## Recent facts
- qa's P0s: helical Sweep 1 0.4.48 -> 0.4.53 2.2x slower = 0.4.53 lacked 895c8e3b (fixed on main; vibe7 same run:
  0.4.48 341-557 ms, 0.4.53 873-977, main 257-326). Dock tab B Puck Huni: fixed by bool's 7ac4debfa + b27a78c40.
- core's housing unify: Rotieren 1 / Boolesch 1 / Bohrung 1 each a USD merge rejected at 4.9e-15 m^3 vs 1e-9 V
  (borderline), three memo-hit tries + one volume: no identical-result cut without moving that threshold.
- gate.sh landed: builds through ~/bin/slot with SLOT_MIN_GB=12 (19afd619), models snapshot per run (a2952b32).

## How to run
- Builds, crate tests, probes: vibe5 (DECISIONS e23d34587): `NOSHAPE_RB_HOST=vibe5 tools/tiers/rb.sh bash -c 'export
  CARGO_BUILD_JOBS=4; cargo test --release -p noshape-kernel --test all <f>'`; rb.sh drops env prefixes (env inside
  `bash -c`). The Mac's one slot only for short crate tests. Don't rb.sh from a worktree while a binary another unit
  still needs is being built from it (rb.sh resyncs the sources).
- Gate (vibe5, one of mine at a time): `systemd-run --unit=bool2-gateNN --collect -p WorkingDirectory=/root/projects/
  noshape-wt/bool2 --setenv=NOSHAPE_GATE_HOST=vibe5 --setenv=HOME=/root bash -c 'bash tools/bool/gate.sh <sha> --jobs 4
  > /root/bool2-scratch/gateNN.out 2>&1'`.
- My own suite pairs: through ~/bin/slot on a `cp -al ~/parity/models` snapshot (NOSHAPE_GATE_MODELS for gate-host.py,
  DECISIONS 11-16); model files change under long runs otherwise.
- Land: clean gate + main green (`/tmp/lead/main-status`), all.rs out, `tools/tiers/land.sh`; `--status`.
- Probes (crates/kernel/tests/xprobe.rs, `--ignored --exact xprobe::<name> --nocapture`, PROBE_CORPUS=<dir>
  PROBE_CASE=<full id>): probe_case_repeat (PROBE_N, NOSHAPE_KERNEL_PROFILE=1, PROBE_SAMPLE=<file> -> vibe5
  ~/agents/bool2-keep/samples.py, callers.py), probe_case_volumes (ours vs OCCT). DBG_AHEAD per-pair OCCT section
  times: /root/bool2-scratch/valve-ahead-dbg.patch. NOSHAPE_OWN_SECTIONS=0 turns our own sections off for A/Bs.
- Recordings: vibe5 ~/agents/bool2-keep/{hel-new (helical 9b68aa2b07dc2986), imp-new (impeller f10ce62e55f0fd49,
  f37371b831c5339f), rec-valve (52970b93111ac42b pattern, e651026a9443a3b6 Extrude 3, 90654ec1c1685fff), sp-rec}.
- Doc timing: qa's saved docs vibe5 ~/qa-parity/saved (copy + -wal/-shm each run); `noshape-cli feature list`,
  regen_ms; under load use user CPU (`/usr/bin/time -f %U`, RAYON_NUM_THREADS=1). Private docs (hats, dock): mode-700
  copies in ~/tmp/bool2, deleted after (udoc run.sh does it). gdb only on a child (ptrace_scope 1): `gdb -batch -ex run
  -ex "thread apply all bt" --args ...` under `timeout -s INT`.

- qa 11-17: in the clean A/B (vibe7, load at most 0.06, v0.4.48 to v0.4.53, median of 3), c2-helical-sweep Sweep 1 took 157 ms on 0.4.48 and 396 ms on 0.4.53 (Ok in both). Your 895c8e3b / e47268f2 are meant to fix this on main. The 11-18 nightly pair (v0.4.55 vs v0.4.56, alternating) will confirm or reopen it.
