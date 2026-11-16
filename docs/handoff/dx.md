# dx handoff: agent API + drawings + data exchange (current state)

State as of 2026-11-16 00:05 UTC. Rewritten, not appended (DECISIONS.md 11-15); history in docs/handoff/archive/dx.md.

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md (generated), drawings and their export (crates/drawing,
crates/model-data/src/drawing), import/export, configurations, Variable Studio, Derived, start window and document
visuals, the parity translator (crates/ops/src/parity.rs) for my studios. The drawing UI is studios' (11-12).
Studio split with asm (lead 11-15): infento 1dc32b / 74d3d6 and bjr-config 5d8bc1 are mine; isaac-ball (3c5c58,
1e81ca, 90f78b) and config-assembly 20e932 are asm's. asm gates parity.rs changes before they land: tell asm.

## In flight
- Configured suppression (live's `BTMSuppressionStateConfigured`), queued 921e815d (checking since 23:49): a feature's own
  suppression is its value at the input's default (`suppressed_by_default`; live's `suppressed` is the feature's flag
  whatever the configuration), each configuration's set by config_configure "suppressed"; `choice_inputs` makes inputs
  from configured suppressions; a studio's default is the configuration live's lines were taken at
  (`__liveConfiguration`); the report counts such a step as suppressed (its add's status isn't compared, no "failed
  to run"). Checked: 5d8bc1 alone at Manufacture=false 107/124 (was 89 against the true lines), Kerf suppressed as
  live (~/tmp/dx/st5d8.log, vibe7); c2-extension-spring A/B 12/12 both; asm no objection. Post-land check
  done: asm-bjr-config with build I 127/137 occurrences, 25/25 mates (was 126/137, 21/22), no Kerf difference. Corpora
  with configured suppressions: asm-bjr-config, asm-config-assembly, asm-xdev-subs (asm's full run), c2-extension-spring.
- 5d8bc1's first difference stays Extrude 55 "vanity plate front" 369/955/601, live 365/943/593 (bool / core, parked).

## Queued to land
- 921e815d configured suppression (checking since 23:49); c2-impeller-loft's split vertex in LIVE_SLIVER_EDGES (00:05). Landed today (latest first): c1af60ab exchange STL 15° over 500 faces + mesh hold; 104e822e assembly studios take their imported Feature Studios (Kerf compiles);
  bb0641ba region faces kept per sketch; a2212ff2 qContainsPoint box skip; 3073e7d8 draft tangent propagation;
  drawing exports 84 / 84 (revision table, note shoulder, splines, jog).

## Open: my studios (studio-alone runs, vibe7)
- bjr-config 5d8bc1: 107/124 at Manufacture=false (23:42, live's retaken lines). First difference Extrude 55 "vanity
  plate front" 369/955/601, live 365/943/593 (bool / core, parked). Kerf Compensation 1 is suppressed at false (as live);
  Kerf compiles (live's 19:19 fetch); core's e138d597 redoes OCCT's empty metre offset in mm (their check: the true
  case 3704.237 -> 3745.156). Hole 95 matches live at false (dx/q-hole95). Text 1 / 2 blocked (private import
  403). Sheet metal model 1's REDG note is harmless (its first query, qSketchRegion, brings both regions; it builds).
  Left: Sketch 39's in-feature cPoint, Decal 1's image.
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
- c2-impeller-loft: f15's Boolean is a known live difference (bool, DECISIONS); 24/48, see Next 2.
- Exchange ("live imports ours") 43/48 (live re-import of the 14:51 files; the gear STL re-imported at 15° 22:0x,
  synced). Left: hose x_t (deliberate split rings); 65-tooth STEP area -0.17 mm² (lead: left); STL rows: gear's Ø0.1
  pin only (ours 12 triangles, live 96: Medium's min facet width; a tiny one gives the pin 92 but the gear 11,182
  triangles for live's 9,000: parked), radijator (ours 17 % fewer triangles,
  under 500 faces: not the angle rule; unexplored), sheet metal plus. Live's REST STL default: 5°, 15° over 500
  exported faces (live2 /root/onshape-research/live/exchange-kw/README.md), the default chord (chord 100 in ours
  matches at 15° within 0.4 % per part); a tiny feature in a large export goes coarser (r 0.05 in a 520 mm studio,
  dx/q1137; none of our rows). Sync the hosts' live-ours*.json after every live re-import.
- cf-spur-gear 13/14 since live's 19:28 refs (Sketch 2 near-singular, "could not be solved" vs "over defined" by
  rounding): skasm's 62893b1ed takes back over-defining model-geometry constraints. Its in-feature cPoint (CAP_EDGE of
  the Spur Gear's own `….blank` extrude) is untranslated: low value.
- drw-centermark regression (qa, greens 45aa7a0e..f8cbf3c6): sent back pointing at studios' f2b9b1087 (take page
  height / click mapping); mine in the range don't touch centermarks.
- Drawing exports 84 / 84 (exp-1115r + the revision / spline / jog runs). drw-broken-out-section compares within ±1 px
  (DECISIONS 11-15). Studios' view fit (69f67433) in.
- Speed: cf-part-draft's Spur Gear 452 -> ~236 ms (live 227). Left there: its opFillet blend 65-120 ms (blend) and
  model-core's own 28 region profiles. cf-snap-hook ×3.4: Snap Hook Booleans + Shell 1 -> bool (took it), fillets ->
  blend (FYI). draft_robust whole-body is_valid: core's 77715251. The kernel profile's per-feature block also counts
  the suite's measurement after the feature (boxes, props, display mesh): compare with the feature's own regen ms.

- Corpus widening (lead via live): live captures 5 public documents for the least-covered features, approved by me
  (before 23:20): c2-surf-workshop-<studio>, c2-surf-intro-<studio>, c2-rib-wrap, c2-sm-tablet-stand,
  c2-sm-rocker-enclosure (in that order; assemblies only if asm wants them), each with its configuration definition,
  `__liveConfiguration` and refs.jsonl; then a second search (sweep twist, loft path/guides, sheet metal rip/corner/hem,
  a configured part). Run them alone once in, sort first differences to owners.

## Next, in order
1. Configured suppression: landed? (`land.sh --status`); then asm-bjr-config's post-land check (vibe5).
2. c2-impeller-loft 26/48 (build L, run on vibe7 by 00:05, ~/tmp/dx/impL.log): f24's split vertex is a known live
   difference (lead, DECISIONS bb69638d4; LIVE_SLIVER_EDGES Part 3 from Extrude 4, Part 1 from Boolean 1, both OTHER,
   1 edge / 1 vertex; live keeps it to f47). First difference f26 "Chamfer Stevenrohr Halterung" (OFFSET_ANGLE 3.5 mm
   20°) Part 1 33/88/56, live 33/86/54: inputs match live's (3 edges, same lengths); live moves Part 3's top lines from
   x=5 to x=1.5 (55.97 -> 59.47 mm), ours don't: blend (its Distance-1 face rule, after 1dc32b's Chamfer 3; it runs
   f26 against that fix next); f27 Fillet 3 fails from it. Translation matches through f27. Save: vibe7
   ~/tmp/dx/saveimp (copy before batches). Its translation notes (Sketch Bohrung Flansch dims, Fillet KsZB, Sketch
   Bohrung Ansaugflansch OFFSET) come later in the doc.
3. (5d8bc1: nothing of mine before Extrude 55.)
4. 74d3d6 Sketch 15 = the general derivedFrom fix (below, in progress): its plane is Plane 1, whose three points
   (imprint INTERSECT VERTEX queries, no live geometry) all became one region (`region_from_query` read any query
   naming SKETCH_ENTITY + FACE + TOPOLOGY + IMPRINT as a region), so Sketch 15 had no plane (the 49.98 mm move).
5. 1dc32b: Fillet 3 once blend lands Chamfer 3.
6. The general derivedFrom / ORIGINAL_DEPENDENCY fix (uncommitted in the dx worktree, above the configured-suppression commit):
   `region_from_query` refuses an EDGE / VERTEX query (test an_imprint_vertex_is_not_a_region passes, vibe7).
   74d3d6 with it (build J, md5 976913a6d920; ~/tmp/dx/st74J.log): still 9/127; Plane 1 now says why it can't map:
   "INTERSECT: SWEPT_FACE: revolve FOxgtrMDqncXEjd_7.holeBodies.* was not built" (a Hole's internal revolve faces
   aren't named by ours; no live geometry for RJCk / RJCc / RJCY). Ask live for those three points' refs once Extrude
   4 matches (Sketch 15 can't be scored before). Model-suite gate A = build I (ops-I7), B = J: vibe6
   `~/bin/slot … gate-tr.sh … rf` (out ~/tmp/dx/gate-tr-rf.out, sums gate-rf-{A,B}.sum / .untr), queued by 23:33 behind
   skasm. Land if the sums are SAME and the untranslated diff is only lines that were wrong regions before.

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
  cli-main is a release build of main 21:45 (rb.sh `cargo build --release -p noshape-cli` on vibe7, 20 min); a stale
  one regenerates without newer fixes. Tools: feature_list, feature_get, feature_edit, feature_rollback,
  query_parts, query_entities, sketch_get, export_parts; `batch` file of {"tool","args"} lines.
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
