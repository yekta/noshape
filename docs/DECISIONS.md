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

- 11-16 STL exports: compared by area, volume and chordal deviation within live's tolerances, not by exact triangle
  count. OCCT BRepMesh's minimum segments per arc / rows per cone (radijator +144/+150) are parked (P3), no OCCT patch.
  A row passes when every live vertex is within live's chord tolerance of ours and the mean deviation |ΔV|/area is
  ≤ 0.01 mm (a tenth of Medium's 0.12 mm chord), watertight.

## Geometry and speed
- 11-07 Every crates/brep or Boolean change passes bool's gate: every corpus + parity56 at 0 OCCT differences.
- 11-14 A faster path lands only if results are identical end to end (topology and feature status in every suite
  document, the hats/dock userdocs, c2-sm-corner-break's 42 faces), not just within tolerance: a different but
  in-tolerance curve can change what later features build (bool2's marcher 11-14, bool's plane x B-spline 11-15).
- 11-15 A bool replay flag (1e-9 volume, history hash) from a more exact curve than OCCT's is accepted when the end-to-end
  check is identical (every suite document's topology and feature status, hats/dock SAME, corner-break 42 faces) and
  the volume is within live's band; any end-to-end difference parks it (rings-loft / knife Mirror 11-15).
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
