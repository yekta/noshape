# Decisions

Standing rulings by the lead. Every agent reads this file at the start of a session and after a compaction. One line
each: date, ruling, reason. Newest at the bottom of its section. A ruling here wins over older notes in a handoff.

## Parity
- 11-03 Parity is measured by the model, interaction and exchange suites (docs/PARITY-SUITES.md, SCORE.md), not the checklists.
- 11-07 Onshape's look and behaviour, never its shortcomings: limits, caps, slowness, failures and lost input are not targets.
- 11-07 Live wins over OCCT: OCCT is only a proxy; a gate exception needs a live capture.
- 11-15 Don't copy a live failure: Sketch Mirror's first click lost after a box select, an unresolved mate connector left
  unresolved, a broken imported Feature Studio stopping the whole Part Studio. Mark such a take's step as a known live
  defect (with the repro), so it doesn't count against us.
- 11-15 Free (unsnapped) sketch clicks: measure readouts compare within ±1 px (one mmPerPixel at that zoom); snapped and
  typed values stay exact. Reason: live's sub-pixel camera puts its clicks up to ~0.5 mm off.
- 11-15 Drawing exports whose geometry comes from screen clicks compare within ±1 px (0.012 in at the take's zoom).
- 11-15 Sketch Offset takes model edges, as Onshape's does (10 OFFSET constraints in 4 documents depended on it).
- 11-15 Agent API Hole: a size the caller gives is never refused or replaced; one that differs from the Standard's table
  is kept and marked overridden ("Parameters have been overridden. Check input."), as live.

- 11-16 Drawing title block: each TITLE line is its own field, as live. A field's pick area covers its own text line and
  the line above (live's offset area), so live's clicks land and a click on the text itself is never lost.
- 11-16 Menu rows for left-out cloud features (Add comment, Share, …) are not drawn or reserved; a take step that differs
  only by such a row is marked a known difference (excluded feature).

- 11-16 Sketch fuzz (830 cases): live calling a line with redundant constraints fully defined (s8, s10, s11) is a live
  defect, not a target. Where free geometry settles after a drag (s1 …) is parked: listed as known, not counted, until a
  general rule for live's drag model is found (no per-case rules). The same holds for where under-defined geometry
  settles in a plain solve; a case live fails to solve at all is a live defect.

- 11-20 (lead) Drawing DXF/DWG exports, like STL (11-16): a row passes when each file's curves lie on the other's within
  0.010 in on paper, per layer, and the curve kinds match where they overlap (LINE / ARC / CIRCLE / SPLINE: a curve live
  writes as a SPLINE or ARC is never our run of LINEs). Entity counts per type are reported, not scored.
- 11-16 STL exports: compared by area, volume and chordal deviation within live's tolerances, not by exact triangle
  count. OCCT BRepMesh's minimum segments per arc / rows per cone (radijator +144/+150) are parked (P3), no OCCT patch.
  A row passes when every live vertex is within live's chord tolerance of ours and the mean deviation |ΔV|/area is
  ≤ 0.01 mm (a tenth of Medium's 0.12 mm chord), watertight.

- 11-18 Duplicate workspace names are allowed, as live. The store keys workspaces by id; ops take a workspace (`branch`)
  by id, or by name when it is unique; an ambiguous name is an error naming the parameter and listing the matching ids.
- 11-22 (lead) An STL (OBJ, 3MF) mesh that closes imports as the model's solids, even where live's import makes sheets
  of it (c2-bottle-split 148, c2-duct-multipart 9: live failing at the job; the 11-17 ruling stands). Live's split
  (each triangle once, joined only across edges two triangles use) applies only to what can't close. The exchange
  suite scores those rows against the model.

## Geometry and speed
- 11-07 Every crates/brep or Boolean change passes bool's gate: every corpus + parity56 at 0 OCCT differences.
- 11-14 A faster path lands only if results are identical end to end (topology and feature status in every suite
  document, the hats/dock userdocs, c2-sm-corner-break's 42 faces), not just within tolerance: a different but
  in-tolerance curve can change what later features build (bool2's marcher 11-14, bool's plane x B-spline 11-15).
- 11-15 A bool replay flag (1e-9 volume, history hash) from a more exact curve than OCCT's is accepted when the end-to-end
  check is identical (every suite document's topology and feature status, hats/dock SAME, corner-break 42 faces) and
  the volume is within live's band; any end-to-end difference parks it (rings-loft / knife Mirror 11-15).
- 11-19 (lead) A gate case with no live document may be an exception without a live capture only when OCCT
  contradicts itself on an exact symmetry (the same arguments rotated or mirrored) and ours gives both copies OCCT's
  other answer (bool2-exact shfall 5101adb8 = 6bb081c8 turned 90°: OCCT 1.93 mm³ off on one). Any other OCCT
  difference still needs live.
- 11-20 (lead) bool's shfall gate is the 103-case OCCT set re-recorded 11-19 at today's tolerances (vibex8
  bool-keep/shrec/rec1). The 11-02 set stays as shfall-1102, a report-only run: each brep land lists any new difference
  from its last run and does not block. Cases where OCCT disagrees with its own recording on every replay are dropped,
  each named with its reason in the corpus README.
- 11-16 A vertex live leaves splitting one edge into two pieces of the same curve (no new geometry; e.g. impeller f24's
  split 47 µm from a corner) is a known live difference when the faces match, the volume is within tolerance and no
  later feature references it. Record it with its measurements in the suite's known-differences table.
- 11-16 An edge that lies inside a face and bounds no face (an INTERNAL leftover, e.g. a hole's old seam) is never shown
  or counted: Parasolid keeps none (a2e4a2 live 32/72/40). A user-document count that drops only by such edges, with
  the same volume and faces, is a fix, not a regression (dock E2 Govde 3104/1760 → 3103/1759).
- 11-15 Edges live adds only inside a sub-micron near-tangent sliver (Parasolid's tolerance choices, e.g. impeller f15)
  are a known live difference when ours is the exact intersection and no later feature references them; chase
  them only if a later feature or a selection depends on them.
  11-17: hulls_apart's face / face margin may be 2 x the two faces' own tolerances (it stands for their face / face
  section only; edge / face and vertex / face checks keep their own): faces 21.6 um apart are not a tangent contact.
- 11-15 OCCT loops must poll the break hook and have a deterministic work bound (patch 0030); never a wall clock.
- 11-15 OCCT patches (0030, 0031, …) land when results are identical end to end, the patch is in patches/occt/ and builds
  with g++-13 and Apple clang; the owner stages built libs (x86-64 on vibe5, arm64 on the Mac) and the lead installs
  them on every host, with backups, before the land.
- 11-15 Draft 1 (1dc32b): OCCT patch + wedge fallback, not wedges-first.
- 11-15 Parsed-std disk cache parked: at most 2 ms gain for 15 MB and more code. Keep it simple.
- 11-16 Speed comparisons count only when both builds run in the same session, alternating, on an otherwise idle host
  (load recorded, near idle throughout; nothing else on it). Never compare runs from different sessions or nights.
  A/Bs on a loaded host are hints for where to look, never results. The nightly window runs the previous and the
  current release back to back.

## Process
- 11-14 Never ask the user anything; decisions go to the lead as "decision needed: …" and work continues.
- 11-14 After a report, go straight on to the next item; reports are 5 lines at most, details in the handoff.
- 11-15 Handoff = current state only (docs/handoff/<agent>.md, at most ~250 lines): in flight, queued to land, open
  questions, next items, how to run. Rewrite it, don't append. History goes to docs/handoff/archive/<agent>.md, which
  nobody reads by default. Update it whenever you land or report, so a compaction never loses anything.
- 11-15 Heavy runs (model/assembly/exchange suites, gates, parity pairs) on a vibe host go through `~/bin/slot` there,
  which takes the host's single suite lock and waits for free memory; never two at once on one host.
- 11-15 Build hosts: bool2 and bool build and gate on vibe5; the Mac's one slot is for crate tests and short builds, never
  a 45-min hold. Light checks (5 app runs, a quit check) don't take ~/bin/slot.
- 11-16 Gate builds (single-unit LTO, ~10 GB) also go through ~/bin/slot (SLOT_MIN_GB=12): earlyoom killed two on vibe5.
- 11-16 Ours-vs-live speed is judged on the Mac (user-class hardware), release build, quiet run, median of 3: the vibe hosts
  are shared build servers, so their absolute times are hints only. Ours-vs-ours A/Bs still follow the same-session rule.
- 11-16 Every ~/bin/slot job runs under `timeout` sized to its expected length (a model suite ≤2 h). A run stuck on one
  document (100 % CPU, no progress) is killed by its owner, the document skipped and its owner told: two runs held the
  vibe5 and vibe6 slots 1.5 h each on c2-config-parts-screws' Split hang.
- 11-16 Long runs (gates, suite pairs, A/Bs) read a snapshot of the corpus taken at their start (`cp -al ~/parity/models
  ~/tmp/<agent>/models-<run>`; the 10-min sync replaces files, so hard links keep the old data), never the live tree:
  bool's 02:05 refs update turned a clean gate FAIL.
- 11-15 No recurring "gate --main" runs: main's watcher and the land queue cover main.
- 11-11 Private user documents (hats, dock, anything in ~/lead-private): never in the repo, artifacts or logs; work only
  on mode-700 copies in ~/tmp/<agent> on the vibe host and delete them afterwards (CLI batches with mutating ops save).
- 11-15 Release cut: a user-document regression on the candidate is reverted on the release branch, never shipped.
- 11-17 A feature that used to fail must never start returning wrong geometry as Ok: an error is better than a silent
  wrong result (f197 Mirror 3 slivers, singularities' point loft). Before landing, compare every changed feature's
  volume / area against live, not only its status.
- 11-17 Suite pairs report every differing feature (NOSHAPE_PARITY_DIFFERING), not just each document's first
  difference: a later feature's broken reference (impeller Move face 2 after 64744410b) hides behind an earlier change.
- 11-17 Don't message paused (stopped) agents: a message wakes them and spends limits. Their P0s go to the lead.
- 11-17 An imported solid whose healed shell isn't closed comes in as sheet bodies, never as an invalid solid with open
  edges. A valid solid stays a solid even where live's import fails and makes sheets (robot-relations' STEP: live 6
  sheets, ours one valid solid; recorded as a live defect, references mapped by geometry). Splitting a valid solid into
  its disjoint pieces waits for live evidence.
- 11-17 Never wait for an idle host: an A/B on a busy host alternates A and B in one run (ABAB…, ≥5 rounds, medians).
  Absolute speed comes from the Mac or vibe7's 02:00–04:00 window. A run fifth in one host's slot queue moves to the
  least-loaded host.
- 11-17 (user) Interaction feel is P0: hover, select, pick priority and sketch interactions (constraints, inference,
  drag, dimensions) as live, within a frame, ahead of geometry tails. In sketch mode the active sketch's entities win the
  hover over planes and faces. Every fix comes with an interaction-feel scenario; a suite that passes while the user hits
  a broken hover is missing coverage, which is part of the bug. live2's lanes serve these captures first.
- 11-17 A Boolean's history may call a shared edge or vertex Modified (same place) where OCCT says Kept (OCCT's choice
  follows its rounding) when the model suite is identical end to end over every feature, hats/dock are SAME, the
  corpus replay differs only in those labels, and a T1 test shows references keep their names through it.
- 11-17 The Mac's one build slot is for speed numbers and Mac-only checks. When a job would queue behind another there,
  build and test on the least-loaded vibe host instead (`uptime` first). Never end a turn to wait for a slot.
- 11-17 (user) The build hosts the lead runs are named vibexN (vibex6, vibex7, vibex8 today); vibeN names are the user's
  own servers. vibe3, vibe4 and vibe5 are gone for good. Land queue: vibe2 (since 21:25). Main's watcher: vibex6. Private user
  documents: vibex7 ~/lead-private only.
- 11-17 21:00 (user) vibe2 (16 cores, 30 GB) is a build host like the others, though it is the user's own server (also
  running the user's Motile): everything of ours under user ngyekta's home and /opt/occt, never /root. 21:25 (user: use it
  as we want): it runs the land queue (land.sh's default); core and blend build on vibex7, bool2 on vibex6.
- 11-17 21:05 (lead) Never kill a process tree from a computed pid: a `kill_tree` on a parent that was pid 1 SIGTERMed
  every process on the agents' machine at 21:00:08 (4 agents, every unit, Chrome, Xvfb). Stop your own units with
  `systemctl stop <unit>`; kill only pids you started ($! or a pid file), after checking the pid is > 1, not an ancestor
  of your shell, and its command line is yours.
- 11-17 A gate or suite pair compares each changed feature's solid validity (BRepCheck) too, not only counts, status,
  volume and area: a feature valid on main and invalid on the branch is a difference, and an invalid solid never lands
  (blend's tighten_tolerances: Halter's input became a 36-face invalid solid while the shown counts looked SAME).
- 11-18 (lead) Help popover texts: our own wording, live's structure. The drawing toolbar's help popovers (and any
  longer help Onshape authors) keep live's title, numbered steps (same count and order), facts and UI names, written
  in our own words; never live's paragraphs copied verbatim (Onshape's authored content). Short UI labels and
  tooltips stay word for word, as parity requires.
- 11-18 (lead) A corpus document whose imported Feature Studio live won't serve (403, not public, no public copy) may
  keep that import as an empty module marked `"stub": true` with the reason, if no feature of the document uses
  anything from it (asm-bjr-config's "Packing"). If a feature uses it, the document leaves the suite with the reason
  recorded.
- 11-18 (user) Speed is judged at human scale, not in milliseconds. Noshape runs on all kinds of machines; the
  comparison with live is "not far behind", not a strict race. A difference counts only when it is >= 100 ms and >= 2x
  (against live or against main) on a whole user action: a click to its result, opening a tab, a document's rebuild.
  Per-feature or per-call millisecond ratios are not targets and are not reported as gaps. A speed regression (P0) is a
  user action or document rebuild >= 20 % and >= 100 ms slower, confirmed by an alternating A/B on one host (>= 5 runs
  each, medians). Single runs on a loaded host are not evidence.
- 11-18 (user) Our own Boolean lands on correctness: identical results on the gate corpora and the hats/dock (live wins
  over OCCT where they differ). Timing only blocks a land when a document rebuild is >= 20 % and >= 100 ms slower
  (A/B-confirmed); this replaces the 11-18 "no document > 5 % slower" rule. bool and bool2 work down the reasons ours
  declines a case to OCCT, ranked by how many recorded cases each reason blocks; their daily report is the share ours
  takes (count) and the number of differing cases.
- 11-19 (lead) "Ours" in that share is every case Noshape code builds instead of OCCT's Boolean: the general Boolean and
  the direct builds (on_face, through_holes, fuse_slabs), reported split by path, e.g. "ours 3121/3477 (general 3026,
  direct 95), OCCT 356, differing 6". "local" runs OCCT's Boolean on the reached faces, so it counts as OCCT. A change
  that only moves cases between our own paths isn't progress.
- 11-19 (lead) Build hosts by load, not by name: before a build or test run, read the 1-min load of vibex7, vibex8 and vibe2
  (`ssh <h> cut -d" " -f1 /proc/loadavg`) and use the lowest that has your worktree (or the Mac's free slot). Naming one host
  for everyone moved the pile-up (vibex7 26 / vibex8 4 at 05:00, then 7 / 34 at 05:30). Private documents stay on vibex7.
- 11-19 (lead) Shell: bool's gate includes the end-to-end Shell check (ours vs OCCT-only, 211 shells, history by geometry);
  no change may add a differing Shell. On Shell's slab path only, vertex-record-only history differences count as
  equivalent (Shell never reads them); everywhere else they count.
- 11-19 (lead) Where live builds a feature and ours errors, a result within 0.1 mm of live's that changes no other
  document lands (pipe Loft: 0.093 mm inside the span, boundaries exact); its parity row stays open for the rest. An
  honest error is not the goal when live succeeds.
- 11-19 (lead) A case where ours differs from OCCT is worse than one it declines: a rule that removes differing cases lands
  even when it also declines correct ones (bool's 50 µm slab rule: 5 differences gone, ~20 correct cases back to OCCT);
  the declined cases are listed in the handoff as the next ones to win back. History-only differences are fixed, not
  declined, where a fix shows 0 new differences on every corpus.
- 11-18 (lead) Hosts: psui builds off vibex6 (the watcher's host) to vibe2; the Mac takes two build slots (12 cores);
  at most one suite, gate or timing pair at a time on an 8-core host (via ~/bin/slot).
- 11-21 (lead) Exported files copy live's content, layers included (DXF/DWG spline polylines on layer 0 as live):
  a layer or entity choice is behaviour, not a shortcoming. Deviate only where live's file is broken (wrong geometry,
  unreadable).
- 11-21 (lead) No kernel limit may change a result with the machine: time limits become work-step counts (blend
  5942f962), the Boolean's memory limit is a flat 8 GB (3/4 of RAM below 11 GB; bool-mem).
- 11-21 (lead) Opening a heavy document shows a stored display result at once (the open target is ≤ 0.5 s): each Part
  Studio's tessellation and part list are saved in the .noshape, keyed by its feature list, configuration and app/kernel
  version; the real regeneration runs in the background and replaces it. A stale cache is never shown (dx).
- 11-21 (user) Hover and selection looks must match live in every combined state, not only one state at a time (the user
  found a midpoint hovered on a selected edge drawn as a cream dot with a black ring; live: a solid orange dot with a soft
  orange halo). P0: a matrix of target (vertex, midpoint, edge, face, plane, origin, mate connector, sketch entities,
  snap glyphs) × owner state (none, hovered, selected, selected + hovered, another selected, in a selection field) ×
  context (Part Studio, sketch, assembly); live captures each cell, psui matches it with a test per cell, qa scores it
  in the interaction feel suite and walks the app as a user.
- 11-21 (user) Use the user's Mac up to half of it: two build slots of 3 jobs at nice 19 (6 of 12 cores); anyone
  may use it (NOSHAPE_RB_HOST=mac) and waits for the slot. Default routing (tools/tiers/rb-hosts) is the vibe hosts;
  vibe2 (16 cores) takes 3 agent build slots of 4 jobs besides the land queue. Older Mac slot rulings are void.
- 11-22 (user: "use the mac properly, without overloading it") The Mac has ONE build slot of 4 jobs at nice 20 that only
  starts while the Mac is calm (1-min load < 6, >= 25 % memory free; env.sh waits otherwise); rb.sh caps transfers to it at
  6 MB/s. Opt in with NOSHAPE_RB_HOST=mac; default routing stays on the vibe hosts. Our app runs on the Mac (GPU timings)
  only while the user has been idle >= 10 min (`ioreg -c IOHIDSystem | awk '/HIDIdleTime/{print int($NF/1e9); exit}'`
  >= 600), one at a time, <= 3 min each. Older Mac rulings are void.
- 11-22 (lead) Our Boolean moves to per-pair tolerances (bool, P1): one global tolerance band (2x the largest input's)
  breaks junctions between precise tools when one input is imported loose (c2-gamma-imported Boolean 1, 87 tools, ~24 s
  on OCCT vs live's ~0.25 s document). Staged, each stage through the full Boolean gate (+0 -0 on every corpus).
- 11-23 (lead) Subtract's cut faces take the target part's colour, as live's (core). They get Boolean-based names for
  that, gated by the Part Studio's feature version: older studios keep their current names so references don't break.
- 11-23 (lead) Importing into an existing Part Studio is Onshape's route: + › Import… (the file becomes its own tab), then a
  Derived of that tab brings its parts in, linked (live import-into-doc-1123: Derived's picker has no Import…).
  No direct "import into this Part Studio" beyond what live offers.
- 11-23 (lead) Sketch Use of an already-used edge and Mirror of an already-mirrored entity make duplicates, as live's
  (skasm): this is behaviour, not a failure, so we match it rather than refuse.
- 11-24 (user) Focus on what individual people using CAD need: UI interactions and feel, sketching, Booleans and core
  modelling, import/export, QA and live captures. Sheet metal (and other niche areas) is parked: no new sheet metal
  work until the lead reopens it; open sheet metal tickets wait.
- 11-24 (lead) Our Boolean's section curves may differ from OCCT's in volume by up to ~1e-6 relative with identical
  topology when ours are the more accurate curves (bool, nuts thread cut: 2e-9..5e-8). Tests and the corpus gate compare
  within live's tolerance band (~1e-6 relative), not OCCT's last digits; no live capture needed below that band.
- 11-25 (lead) A mate between two entities of one instance (e.g. a Tangent of two faces of one part) is accepted by
  the agent API as live's REST accepts it; the dialog takes it only where live's dialog does (skasm, OPS-TANGENT-MATE).
- 11-25 (lead) A pattern of fewer than 16 Boolean tools fuses at once rather than through the trimmed fuse (bool,
  impeller seat Extrude 0.9-1.3 s -> 0.22-0.38 s). A corpus case whose raw fuse differs only before the feature's unify
  is a gate exception when a T1 pins the feature's final result (faces, volume) as recorded (bool-corpus 58c61d: 65 raw,
  56 after unify, as recorded); the model suite vs live and parity56 stay at 0 differences.
- 11-26 (lead) bool starts noshape's own free-form × free-form surface intersection (B-spline / offset / extrusion pairs), staged: an intersector checked against OCCT on every replay pair lands first unused; it is switched on per pair family only at 0 differences in the gate, parity56 and the knurls. First stage time-boxed to ~2 days with a report (pairs matched, time vs OCCT, user documents moved). Reverses the 11-24 hold now that the remaining ≥100 ms / ≥2× Boolean rows all need it.
- 11-26 (lead) The 11-15/16 sub-pixel camera ruling covers sketch geometry placed by clicks too: a ui-parity point within 2 screen pixels of live's counts as equal (psui, SKETCH-CLICK-OFFSET-EMPTY-TOP). It does not cover typed or snapped values, which must match exactly.
- 11-27 (lead) Geometry parity is judged per face (area and edges matched to live's faces), not by document totals: a change that brings more faces to live's while a document's edge/vertex totals move away from live's lands, because totals that matched before were cancellation (bool slit fill 0d6fc6b9: gamma faces matched 666 → 681, totals 2403 → 2411 vs live 2405). The remaining per-face differences stay open rows.
- 11-27 (lead) A test's expectation is loosened only when the old one was wrong against live (or, without a live number, against an independent check), and the commit says which. It is never loosened so a fix passes: find why the fix moves the number. bool 2120ccaae loosened a volume bound for a speed fix; the cause was pcurve gaps in the new integral, fixed in 73c26f0c2 and the bound restored (805598536).
- 11-27 (lead) Sketch Select ▸ on used (projected) curves: live offers only "Create selection…" there; ours keeps Tangent connected / Equal / Parallel working on them (skasm b43d5331, after the user hit it). A missing command is a shortcoming, not a target; everything else on that menu matches live.
- 11-27 (lead) vibe2 runs the land queue and psui's builds only: no qa suites there. earlyoom killed the queue's part-studio-ui tests (5.2–5.4 GB RSS) twice on 11-27 while a parity_assemblies run (6.7 GB) shared the host, failing two lands as "tests". A land that fails with a test binary's SIGTERM is checked against `journalctl -u earlyoom` before anyone debugs it.
- 11-27 (user) vibex6 is gone. Hosts: vibe2 = the land queue + psui's builds (no qa suites); vibex7 = main's watcher (moved 18:2x), core's builds, perf.sh, qa's 02:00–04:05 speed window; vibex8 = bool, skasm, qa's suites and the nightly T2 (00:30 UTC, via ~/bin/slot); the Mac through its gated slot. Heavy runs on vibex7/vibex8 go through ~/bin/slot, one at a time.
- 11-27 (lead) A start or e2e check whose geometry is a known gap from live may widen its tolerance only as a named known gap: the ticket id beside the tolerance in the test, tightened back when the ticket closes (live's spline-fillet-asm start, 1.08 mm³ short of live, SPLINE-FILLET-ASM-R2-END for core). Same rule as gate exceptions.
- 11-27 (lead) Export settings are remembered as live remembers them per account: Format, Options, Resolution, Units and Y axis up, per user in the app's preferences on this machine; "Export unique parts as individual files" is not remembered (live behaviours "Depth batch 8").
