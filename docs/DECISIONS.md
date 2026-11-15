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

## Geometry and speed
- 11-07 Every crates/brep or Boolean change passes bool's gate: every corpus + parity56 at 0 OCCT differences.
- 11-14 A faster path lands only if results are identical end to end (topology and feature status in every suite
  document, the hats/dock userdocs, c2-sm-corner-break's 42 faces), not just within tolerance: a different but
  in-tolerance curve can change what later features build (bool2's marcher 11-14, bool's plane x B-spline 11-15).
- 11-15 A bool replay flag (1e-9 volume, history hash) from a more exact curve than OCCT's is accepted when the end-to-end
  check is identical (every suite document's topology and feature status, hats/dock SAME, corner-break 42 faces) and
  the volume is within live's band; any end-to-end difference parks it (rings-loft / knife Mirror 11-15).
- 11-15 Edges live adds only inside a sub-micron near-tangent sliver (Parasolid's tolerance choices, e.g. impeller f15)
  are a known live difference when ours is the exact intersection and no later feature references them; chase
  them only if a later feature or a selection depends on them.
- 11-15 OCCT loops must poll the break hook and have a deterministic work bound (patch 0030); never a wall clock.
- 11-15 OCCT patches (0030, 0031, …) land when results are identical end to end, the patch is in patches/occt/ and builds
  with g++-13 and Apple clang; the owner stages built libs (x86-64 on vibe5, arm64 on the Mac) and the lead installs
  them on every host, with backups, before the land.
- 11-15 Draft 1 (1dc32b): OCCT patch + wedge fallback, not wedges-first.
- 11-15 Parsed-std disk cache parked: at most 2 ms gain for 15 MB and more code. Keep it simple.
- 11-15 Speed claims: relative A/B in one run on any host; absolute numbers only from the 02:00 vibe7 window or a quiet
  host; vibe timings under load are candidates, not results.

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
- 11-15 No recurring "gate --main" runs: main's watcher and the land queue cover main.
- 11-11 Private user documents (hats, dock, anything in ~/lead-private): never in the repo, artifacts or logs; work only
  on mode-700 copies in ~/tmp/<agent> on the vibe host and delete them afterwards (CLI batches with mutating ops save).
- 11-15 Release cut: a user-document regression on the candidate is reverted on the release branch, never shipped.
