# bool2: current state (11-16 01:58 UTC)

Second agent on noshape's own Boolean (crates/brep, crates/kernel/cpp/rust_boolean.cpp, boolean.cpp's unify). A case
is taken only where it equals OCCT (or live, with a capture); results identical end to end (docs/DECISIONS.md 11-14).
History: docs/handoff/archive/bool2.md. Rulings: docs/DECISIONS.md.

## In flight
- 68c1942bd (helical unify: a cylinder group whose boundary meets the seam's line between vb and vt isn't projected;
  Sweep 1's unify 71-77 -> 23-26 ms) queued 11-16: gate57 clean (replay 3026 -> 3026 taken, 5 -> 5 differing; suite
  98 docs 883/935 both), hats/dock SAME against main. OCCT patch 0031's commit waits on branch bool2-oc31 (ae1731fbf).
- OCCT patch 0031 (patches/occt/0031-topoltool-bspline-samples-grid.patch; lead approved 11-15 on 0030's terms):
  Adaptor3d_TopolTool::BSplSamplePnts evaluates its grid once (c2-impeller-loft Extrude 4's Boolean 125-160 -> 73-81
  ms). Libraries staged: vibe5 ~/agents/bool2-keep/occt-0031 (/opt/occt with the patched object in libTKG3d.a; GCC 15
  -march=x86-64 with OCCT's flags incl. -DNo_Exception -DOCCT_NO_PLUGINS, no ymm/vfmadd; g++-13 compiles it clean),
  Mac ~/noshape-host/occt-0031 (arm64, Apple clang, the build tree's own command). Checks: corpora replay against
  gate57's 68c1942bd identical (/root/bool2-scratch/occt31-gate.out); suite 98 common docs unchanged, 12 docs new
  since; A's suite run later differed on 20 c2-surf docs only because those model files were rewritten at 01:10 UTC
  between the runs: unit bool2-occt31-suite2 (/root/bool2-scratch/occt31-suite2.out) runs A and B side by side.
  Unit bool2-occt31-rest (/root/bool2-scratch/occt31-rest.out): hats/dock A vs B and main vs A
  (~/tmp/bool2/udoc/run.sh, copies deleted after). Then: tell the lead the commit; the lead installs the libs on
  vibe3/5/6/7 (+ /root/occt); then land.sh (all.rs out).
- Lead's gate.sh change (builds through ~/bin/slot, SLOT_MIN_GB=12) queued as 46ce12b0 from worktree
  /root/projects/noshape-wt/bool2-tools (branch bool2-gate-slot); remove that worktree once landed.
- core's housing unify item (11-16): CLI path Rotieren 1 unify 24-26 ms, Boolesch 1 13-24, Bohrung 1 ~10; each a USD
  merge rejected at 4.9e-15 m³ vs 1e-9 V (borderline), three memo-hit tries + one volume (9 ms). No identical-result
  cut without moving that threshold (needs live's counts). Reported to core.
- Worktree /root/projects/noshape-wt/bool2 (branch bool2-chamfer), scratch /root/bool2-scratch. Uncommitted on
  purpose: crates/kernel/tests/all.rs registers the scratch probes (`mod sampler; mod xprobe;`, copy in
  /root/bool2-scratch/all-with-probe.rs), crates/kernel/tests/{xprobe,sampler}.rs, scratch-docs/ (qa's docs, the valve
  recording). Take all.rs out (`git checkout crates/kernel/tests/all.rs`) before land.sh, put it back after.
- Section diagnostics patch: /root/bool2-scratch/valve-ahead-dbg.patch (DBG_AHEAD: each OCCT pair's time and curves;
  DBG_CLIP: cone x cone with the bigger face clipped).
- qa's dock P0 (tab "B - Cakil Tasi", Puck Huni 941 ms on v0.4.52): already fixed on main by bool's 7ac4debfa +
  b27a78c40 (Puck Huni 146-250 ms on vibe5 under load). Tab B's top-10 sent to qa 11-15. Profile script: vibe5
  ~/tmp/bool2/dock-prof.sh <cli> <tag> (mode-700 copy, deleted after; copy in /root/bool2-scratch/bin).
- Recordings: valve vibe5 ~/agents/bool2-keep/rec-valve (52970b93111ac42b = Circular pattern 1), doc in valve-doc.

## Landed today (11-15)
- 897b690f, 7eedbae5: chamfer fallback, sphere sections, trimmed conversion, unify order, near-whole circle, exact
  boxes kept on the part, Bézier pieces for rays.
- 895c8e3b: c2-helical-sweep's Sweep 1 cut on ours again (OCCT's 290 -> 35-60 ms): a section's end at a seam put on
  the seam, two sections' ends at one edge with the same third face one vertex, made cylinder faces allowed past a
  turn by their edges' tolerance; c2-valve-config's Circular pattern 1 checks only each step's made faces past the
  first (50-75 -> 13-22 ms).
- 4730d0c2: the end merge looks only among nearby ends (895c8e3b had made k25-15 3 % slower; level again, Mac CPU A/B).

## Open decisions (with the lead)
- Chamfer 26 builds since 897b690f; core's Delete face 2 (OCCT defeaturing) then takes 17-26 s instead of 4-7 s, the
  impeller 29-35 s a regen instead of 20-25. Keep the chamfer and core takes Delete face 2, or hold it? Asked 11-15.

## Where qa's documents stand (Mac, main at 4730d0c2, median of 3; live from qa's 08:36 run)
- c2-helical-sweep 72 ms (live 40): Sweep 1 74 ms = Boolean 34.5 (OCCT's sections ahead 21.8) + unify 24 (of it
  merge_cylinder_groups 17.3) + split 7.6 + pipe 2.7.
- c2-housing-draft 71 ms (live 105).
- c2-valve-config 267 ms (live 175): Circular pattern 1 140, Extrude 3 55, Extrude 2 46.

## Next items, in order
1. OCCT patch 0031 (above): A/B, gate, decision to the lead.
2. Valve Circular pattern 1: per step 2 cone x cone OCCT pairs (the part's small cone patches at the 2 m tool cone's
   apex), 6-18 ms each (one 45 ms outlier); over 7 steps 200 pairs, 290 ms CPU, 9 with curves. Clipping the tool face
   to the patch's box doesn't help (apex: all angles; 6-9 ms). The empty ones (5 cone x cone 75 ms, 21 revolution x
   cone 41 ms, 161 extrusion x cone 53 ms) would need an exact apart test. Parked.
3. Helical Sweep 1's Boolean: 6 OCCT pairs, cylinder x the sweep's B-spline (10 ms each, 10 helix curves) and plane
   x B-spline (3-5 ms): exact curves, bool's plane x B-spline area. Parked.
4. Valve Extrude 3 (e5811c1b): 23 pairs revolved B-spline profile x B-spline extrusion in OCCT's walk, ~33 ms CPU
   each: our own revolution x extrusion section (not started).
5. Impeller chamfer 26's cut (f37371b8, OCCT 0.9 s): the run-on tool's end-cap lens; needs the arc, the B-spline edge
   and the strip's edge taken as one common block.
6. Sphere x B-spline (cf-surface-pattern d6500b16, 2.2 s): Rust part 1.4 s in 2D curve sampling of refitted sections.

## How to run
- Builds, crate tests, probes: vibe5 (DECISIONS e23d34587): `NOSHAPE_RB_HOST=vibe5 tools/tiers/rb.sh bash -c 'export
  CARGO_BUILD_JOBS=4; cargo test --release -p noshape-kernel --test all <f>'`. The Mac's one slot only for short crate
  tests. rb.sh drops env prefixes: put env inside `bash -c '...'`.
- Gate (vibe5, one of mine at a time; its replays go through ~/bin/slot there): `systemd-run --unit=bool2-gateNN
  --collect -p WorkingDirectory=/root/projects/noshape-wt/bool2 --setenv=NOSHAPE_GATE_HOST=vibe5 --setenv=HOME=/root
  bash -c 'bash tools/bool/gate.sh <sha> --jobs 4 > /root/bool2-scratch/gateNN.out 2>&1'`; read with
  `grep -E 'VERDICT|^(replay|model suite|speed)'`. "CHECK: the knurls not run" is normal on vibe5. --jobs 6 got
  rustc killed by earlyoom under load.
- Land: clean gate + main green (`/tmp/lead/main-status`), all.rs out, `tools/tiers/land.sh`; status
  `tools/tiers/land.sh --status`. land.sh's clippy precheck runs on the crates touched.
- Probes (crates/kernel/tests/xprobe.rs, `--ignored --exact xprobe::<name> --nocapture`, PROBE_CORPUS=<dir>
  PROBE_CASE=<id>): probe_case_repeat (PROBE_N, NOSHAPE_KERNEL_PROFILE=1 for the kernel timers, PROBE_SAMPLE=<file>
  for sampler.rs stacks -> /root/bool2-scratch/bin/samples.py, callers.py), probe_case_volumes (ours vs OCCT counts,
  validity, volume), probe_case_small_faces, probe_case_clipped. OCCT frames in the sampler's stacks are unreliable.
- Recordings: vibe5 ~/agents/bool2-keep/{hel-new (helical 9b68aa2b), imp-new (impeller f10ce62e, f37371b8), sp-rec
  (sphere d6500b16, e49701dd)}, ~/tmp/bool2/rec-valve (valve d561e545, e5811c1b; ~/tmp is pruned). Record with
  NOSHAPE_RECORD_BOOLEANS=<dir> NOSHAPE_RECORD_EXACT=1 on the CLI.
- Doc timing (qa's repro): fresh copy of the .noshape (+ -wal/-shm) each run, `noshape-cli feature list --doc <copy>`,
  regen_ms; median of 3. On the Mac: release CLI `cargo build --release -p noshape-cli` via rb.sh, docs synced in
  scratch-docs/, scratch-docs/sum.py sums regen_ms. Speed claims: relative A/B in one run (DECISIONS 11-15); on vibe5
  under load use user CPU (`/usr/bin/time -f %U`, RAYON_NUM_THREADS=1) or the Mac.
