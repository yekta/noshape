# dx handoff: agent API + drawings + data exchange (current state)

State as of 2026-11-15 19:22 UTC. Rewritten, not appended (DECISIONS.md 11-15); history in docs/handoff/archive/dx.md.

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md (generated), drawings and their export (crates/drawing,
crates/model-data/src/drawing), import/export, configurations, Variable Studio, Derived, start window and document
visuals, the parity translator (crates/ops/src/parity.rs) for my studios. The drawing UI is studios' (11-12).
Studio split with asm (lead 11-15): infento 1dc32b / 74d3d6 and bjr-config 5d8bc1 are mine; isaac-ball (3c5c58,
1e81ca, 90f78b) and config-assembly 20e932 are asm's. asm gates parity.rs changes before they land: tell asm.

## In flight
- Worktree /root/projects/noshape-wt/dx (branch dx-bosdxf), uncommitted: crates/ops/tests/parity.rs
  `add_imported_studios`: an assembly corpus studio takes the Feature Studios fetched beside it (studios/<el>.fs, as
  `read_model` reads them) that its own sources import, transitively; test
  `assembly_studios_take_the_feature_studios_theirs_import` (Mac run in progress). Then: tell asm, land, rerun 5d8bc1.
- Live fetched (19:19 request) asm-bjr-config/studios: Kerf Compensation's imports dcf1d11b… (its own document
  57df36a8…), af743c73… Functions, ec739638… projectiontype.gen.fs, Laser Utils 22d17eb9… at Kerf's version
  da896789…, icons cc74e32c… / 8842c738… (.svg; read_studios only takes .blob: icons stay placeholders). Text
  (Pascoe)'s import a75ab01d… is in a private document (403): Text 1 / Text 2 can't compile from the corpus.

## Queued to land
- Nothing. Landed today (latest first): bb0641ba region faces kept per sketch; a2212ff2 qContainsPoint box skip;
  3073e7d8 draft tangent propagation; drawing exports 84 / 84 (revision table, note shoulder, splines, jog).

## Open: my studios (studio-alone runs, vibe7)
- bjr-config 5d8bc1: 89/124 on main 19:03 (core's Sweep 1 722481801 in). First difference Extrude 55 "vanity plate
  front" 369/955/601, live 365/943/593 (bool / core, parked). Custom features 0/3: Kerf after the loader change, Text
  blocked (private import). Translation left: Sheet metal model 1's region query (REDG: "no region of ours inside
  sketch FAjPmG3yqtXGKDE_21's curves []"), Sketch 39's in-feature cPoint (no live geometry), Decal 1's image
  (BTMParameterReferenceImage not translated). Sketch 29's 40 dims / Sketch 28 / 33 are taken back to live's geometry
  (0 mm left): harmless. Extrude 4 / 24 are suppressed in live (their sketches come later): harmless.
- infento 1dc32b: pulley = live through Fillet 5 (live's refs in studios/1dc32b….refs.jsonl, 15 lines, all hosts) and
  Draft 1 (core, -1306.543 mm³). Chamfer 3: blend (blend-ch3-wip, waits on live's mirrored-body side rule); Fillet 3
  after it.
- infento 74d3d6: 9/127 with loose boxes; first difference Extrude 4 "Knurled disc" 6/11/6, live 4/4/0 (core's up-to
  imprints, parked). Later: Sketch 15 (DIAMETER unsolved, constraints moved 49.98 mm, taken back).
- Studio refs' boxes are live's loose ones (evBox3d tight=false; only lines with "tight" in `source` are tight): the
  studio scripts run loosen.py (sets `looseBoxes`). The assembly suite matches studio parts by volume / centroid,
  not boxes: nothing to change there.
- General translator fix (open, mine): derivedFrom + ORIGINAL_DEPENDENCY queries resolve to the wrong type (1dc32b's
  Chamfer 4 EDGE became a hole wall, Fillet 5's became regions; patched with live's refs). They resolve silently, so
  find one through a first difference before working on it. A SWEPT_FACE / SWEPT_BODY of an opSweep has no named path
  (only opExtrude / opRevolve): live's per-feature geometry maps them when present (5d8bc1 Shell 1 does).

## Open: other
- c2-impeller-loft (core 19:1x, FYI): first difference feature 15 Boolean Flansch+Gehäuse 19/53/36, live 19/56/39,
  both inputs matching (looks like the Boolean's edge merging: bool / core). Translation notes there: Sketch Bohrung
  Flansch (3 dims unsolved, 2.4 mm back), Fillet Flansch zum Stroemungskanal (KsZB: no edge), Sketch Bohrung
  Ansaugflansch (2 OFFSET, 44 mm back). Save: vibe7 ~/tmp/core/imp/c2-impeller-loft.noshape. Check and hand on.
- Exchange ("live imports ours") 43/48 (live re-import of the 14:51 files). Left, all parked: hose x_t (deliberate
  split rings); 65-tooth STEP area -0.17 mm² (lead: left); STL rows gear / radijator / sheet metal plus. Live's REST
  STL default (3138, dx/q1137/summary.json): 5° on every part, no chord cap; only a tiny feature in a large export goes
  coarser (r 0.05 in a 520 mm studio: 17 points). The suite's STL args (5°, chord 100) are that rule. Unexplained:
  live's 20-tooth gear's Ø10 keyway bore at 15° (23 points over 330°, ours 67); no size rule fits (a lone r 5
  cylinder gets 5°): maybe a per-body facet budget. Sync the hosts' live-ours*.json after every live re-import.
- Drawing exports 84 / 84 (exp-1115r + the revision / spline / jog runs). drw-broken-out-section compares within ±1 px
  (DECISIONS 11-15). Studios' view fit (69f67433) in.
- Speed: cf-part-draft's Spur Gear 452 -> ~236 ms (live 227). Left there: its opFillet blend 65-120 ms (blend) and
  model-core's own 28 region profiles. cf-snap-hook ×3.4: Snap Hook Booleans + Shell 1 -> bool (took it), fillets ->
  blend (FYI). draft_robust whole-body is_valid: core's 77715251. The kernel profile's per-feature block also counts
  the suite's measurement after the feature (boxes, props, display mesh): compare with the feature's own regen ms.

## Next, in order
1. Land the assembly studio Feature Studio loader (tell asm first); rerun 5d8bc1 through `~/bin/slot`; Kerf's 1
   feature should compile.
2. c2-impeller-loft: confirm the Boolean is the first difference on main, hand it to bool / core with the repro.
3. 5d8bc1: Sheet metal model 1's region query (REDG), Sketch 39's cPoint.
4. 74d3d6: Sketch 15's unsolved diameter (after core's Extrude 4, or alone if it matters downstream).
5. 1dc32b: Fillet 3 once blend lands Chamfer 3.
6. The general derivedFrom / ORIGINAL_DEPENDENCY fix, on a case from 3-5.

## How to run
- Builds and crate tests on the Mac: `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p <crate> …` (one slot).
  Linux ops binaries for vibe runs: `NOSHAPE_RB_HOST=vibe7 tools/tiers/rb.sh cargo test -p noshape-ops --test all
  --no-run`, then copy ~/agents/dx/target/debug/deps/all-<hash> to ~/agents/dx-keep/ops-exch (studio scripts' binary)
  or ~/tmp/dx/ops-A / ops-B for an A/B. rb.sh keeps mtimes: `touch` an edited file before the A rebuild, check md5.
  An A/B baseline is rebuilt from the same base (land.sh rebases).
- Heavy runs on vibe hosts through `~/bin/slot <cmd>` (one per host, waits for free memory; DECISIONS 11-15).
- Studio-alone runs (vibe7, scripts in ~/agents/dx-keep/bin, model dirs ~/agents/dx-keep/m*/st-*): st5d8.sh,
  st74.sh, st1dc14.sh copy the corpus studio into a model dir, run loosen.py, save to ~/tmp/dx/save*; one line of
  result in ~/tmp/dx/st*.log (grep "features match").
- Parity models: NOSHAPE_PARITY_ONLY=<docs,…> NOSHAPE_PARITY_MODELS=~/parity/models <ops binary> parity::parity_models
  --ignored --nocapture; NOSHAPE_PARITY_SAVE=<dir> saves the translated document; NOSHAPE_PARITY_TRACE=1;
  NOSHAPE_KERNEL_PROFILE=1 (kernel calls per feature), NOSHAPE_FS_PROFILE=fn (FeatureScript lines). A saved document:
  `~/agents/dx-keep/cli-main call feature_get '{"doc":…,"feature":…}'` on a copy (CLI batches save in place).
- Gates for custom-host changes: dx-keep/bin/gate-rf.sh (all 47 cf models, A then B, scores diffed).
- Exchange suite: dx-keep/bin/exch.sh (NOSHAPE_EXCHANGE=~/parity/exchange, parity::exchange_round_trip).
  /root/dx-scratch/bin/stlinfo.py <stl>: triangles, box.
- Drawing exports: /root/dx-scratch/exprun.sh <out> <scenario…> (EXPHOST=vibe7; builds the app on the host, runs
  ui-parity, compares with tools/drawing-export/compare.py against /root/parity/ui/live).
- perf gives no callers on these binaries and vibe7 has no gdb: attribute by NOSHAPE_KERNEL_PROFILE counts, or a
  temporary backtrace build.
- vibe7 → vibe6 scp fails: relay through /root/dx-scratch/bin-relay. Host ~/tmp is pruned after 6-12 h: keep scripts
  and binaries in ~/agents/dx-keep.
- Agent ids (message_agent needs them full): core 6692ae35-758a-4442-81c8-29f67be2e21a,
  asm 28d8f565-94b2-4f19-9946-51740c12fad5, blend 2e21b490-fe92-4d07-add2-54a85a9e3145, bool
  c25aee1d-f0a9-4fab-ac99-acca645c81c7, live b89af730-e8c6-4234-b83e-00a9e1ba6acc, live2
  c42ce02b-3429-4294-861b-ccbc6cbb41d2, studios 08f83230-1122-4c3c-a185-edf9f225d834, skasm
  a09d30a7-95db-42d1-8f2d-946c6fb8f232.
- The archive's "Parity translator changes by asm" section lists asm's translator fixes one line each (lead 11-07).
