# bool2: current state (11-16 11:32 UTC, paused by the lead until ~11-17 10:15 UTC)

Second agent on noshape's own Boolean (crates/brep, crates/kernel/cpp/rust_boolean.cpp, boolean.cpp's unify). A case
is taken only where it equals OCCT (or live, with a capture); results identical end to end (docs/DECISIONS.md 11-14).
History: docs/handoff/archive/bool2.md. Rulings: docs/DECISIONS.md (read it first).

## Unlanded work
- Worktree /root/projects/noshape-wt/bool2, branch bool2-chamfer, on main c899c2bcf, three commits (not queued):
  - 7397e21db: an extrusion across a surface of revolution's axis is sectioned on noshape's own (quadric_across in
    rust_boolean.cpp: each ruling meets the profile's radius at its height; Newton in a bracket for the height).
  - ce7f948bd: the same across a cone's axis (own_sections' cone branch sends it to quadric_across).
    Together: c2-valve-config Extrude 3's cut e651026a CPU 1.98 -> 0.90 s per 10 runs, 36/90/59 valid, volume 3.7e-12
    off OCCT's; kernel tests built with g++-13 pass (463).
  - 5e1aa70b5: unify_in's reordered tries integrate volumes only for a try that left fewer faces (valve's
    unify.reordered 62-142 -> 17-70 ms in Extrude 1 / 2 / Circular pattern 1, 10/10 features Ok).
  Uncommitted on purpose: crates/kernel/tests/all.rs registers the scratch probes (`mod sampler; mod xprobe;`, copy in
  /root/bool2-scratch/all-with-probe.rs), crates/kernel/tests/{xprobe,sampler}.rs, scratch-docs/ (qa's docs, valve
  recording). Take all.rs out (`git checkout crates/kernel/tests/all.rs`) before land.sh, put it back after.
- Worktree /root/projects/noshape-wt/bool2-oc31, branch bool2-oc31 = 7397e21db + two commits (not queued; land only
  after the lead installs the libraries):
  - aa137dcc7: OCCT patch 0031 (patches/occt/0031-topoltool-bspline-samples-grid.patch): Adaptor3d_TopolTool::
    BSplSamplePnts evaluates its grid once (c2-impeller-loft Extrude 4's Boolean 125-160 -> 73-81 ms).
  - 0938173f0: OCCT patch 0032 (patches/occt/0032-unify-wire-bound.patch): USD's IntUnifyFaces wire loop bounded by
    work (2 (edges + removed edges) + 16 turns, break hook polled), throwing "noshape: a wire that never closes";
    op_unify (boolean.cpp) catches only that and keeps its input. core's P0: c2-config-parts-screws Revolve 9 grew to
    15 GB on an invalid thread; with 0032 it stays ~250 MB (the doc's later Split hang is bool's 979de111a, dx's P0).
  Lead approved 0031 on 0030's terms (11-15); 0032 rides with it. Staged libraries (both patches): vibe5
  ~/agents/bool2-keep/occt-0032 (= /opt/occt with patched objects in lib/libTKG3d.a and lib/libTKShHealing.a; GCC 15
  -march=x86-64 with OCCT's flags incl. -DNo_Exception -DOCCT_NO_PLUGINS, no ymm/vfmadd; g++-13 compiles both clean),
  Mac ~/noshape-host/occt-0032 (arm64, Apple clang with the build tree's own command). (occt-0031 dirs: 0031 only.)

## Running (results land in /root/bool2-scratch)
- Unit bool2-gate60 (gate on 5e1aa70b5 vs main c899c2bcf; built 10:37, replays/suite running): gate60.out, read with
  `grep -E 'VERDICT|^(replay|model suite|speed)'`. Clean ("CHECK: the knurls not run" is normal on vibe5) + main
  green -> land bool2-chamfer with land.sh (all.rs out).
- Unit bool2-oc32b (0031+0032 identical checks, script bin/oc32-cmp.sh): oc32-cmp.out. A = ~/agents/bool2-keep/
  {a-kern,a-ops,cli-a2} (bool2 at 7397e21db, /opt/occt), B = {b-kern,b-ops,cli-b2} (bool2-oc31, occt-0032). Corpora
  replay + model suite side by side on a models snapshot through ~/bin/slot (queued behind gate60), then hats/dock A vs
  B (~/tmp/bool2/udoc/run.sh; backup in ~/agents/bool2-keep/udoc-bak). Earlier, 0031 alone: replay identical to
  gate57's, hats/dock SAME 12/12. Done when oc32-cmp.out ends with OC32-DONE: replay 0 differences, suite document by
  document the same (c2-config-parts-screws timed out on A as on B before 0032: B may now finish it, that's 0032's
  fix), hats/dock SAME -> tell the lead "0031+0032 identical" + both lib dirs; he installs on vibe3/5/6/7 + Mac; then
  land.sh from the bool2-oc31 worktree (rebase on main; all.rs isn't modified there).
- Waiters in the paused session are gone; check the units with `systemctl is-active bool2-gate60 bool2-oc32b`.

## Open decisions (with the lead)
- Chamfer 26 builds since 897b690f; core's Delete face 2 (OCCT defeaturing) then takes 17-26 s instead of 4-7 s.
  Keep the chamfer and core takes Delete face 2, or hold it? Asked 11-15.

## Next items, in order
1. Land bool2-chamfer (gate60) and the OCCT patches (after the checks + the lead's install).
2. Impeller "Chamfer Stevenrohr Halterung" (960 ms on vibe5: op_boolean 634, OCCT's builder 332): chamfer 26's cut
   f37371b8 declines "a piece's point on another solid's face" (boolean.rs 3885, state_at 6270-6420): the run-on
   tool's end cap, a 0.3 mm lens across the strip (B-spline face 3) and the face its arc lies on, all within 12 um.
   Needs the tool's arc, its B-spline edge and the body's strip/face edge taken as one common block (archive 11-15).
3. Valve Circular pattern 1 (Boolean ~140 ms Mac): per step 2 cone x cone OCCT pairs at the 2 m tool cones' apex,
   6-18 ms each; 200 pairs over 7 steps, 9 with curves; clipping the tool face doesn't help. Parked.
4. Helical Sweep 1's Boolean: 6 OCCT pairs (cylinder x the sweep's B-spline, plane x B-spline): exact curves,
   bool's plane x B-spline area. Parked. Its unify (15-16 ms) is ~20 ms over 0.4.48's (another Boolean result): low.
5. Impeller Extrude 4 after 0031: OCCT's PointDepart 16 %, BRepCheck 13 % (samples).
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
