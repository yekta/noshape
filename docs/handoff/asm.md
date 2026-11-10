# Handoff: asm (assemblies), from 2026-11-06

Taken over from skasm on 2026-11-06 (skasm keeps sketches and sheet metal). Area: crates/assembly, crates/assembly-ui,
the assembly ops in crates/ops and their e2e tests, the assembly (asm-*) scenarios of the ui-parity suite.

Worktree /root/projects/noshape-wt/asm (main, detached), target dir /root/target-asm, display :105, scratch
/root/asm-scratch (bin/: scenario generators). Builds and tests: `tools/tiers/rb.sh …` (vibe3, 8 cores,
rb-hosts `asm*`). The app here: `tools/tiers/rb.sh --get noshape cargo build -p noshape`, then
`tools/ui-parity/run.py --app /root/target-asm/debug/noshape --display :105 <names>`.

Older assembly history: docs/handoff/skasm.md (assembly notes, speed numbers, fuzz) and assembly-behaviour.md.

## From dx (2026-11-10 09:05 UTC, lead, asm paused): variable renames reach Assemblies

- A Variable Studio variable renamed now renames `#old` in every Assembly that sees the studio (they were skipped:
  mates kept `#w` and failed as undefined): `Assembly::rename_variable_refs` (model-data assembly/mod.rs) covers
  mates' offsets, limits, moved connectors and Unsuppressed if, assembly mate connectors, relations, linear /
  circular pattern spacing, configuration variables' defaults and ranges, configured values and instances'
  configuration values. e2e `variable_studio_rename_reaches_assemblies`. `assembly_list` now reports a linear /
  circular pattern's `distance` (or angle) text and `count`.
- Configuration variables (live 2458, done 11-10 11:45): `#` names an input's FeatureScript id, not its shown name
  (`Configuration::variable_exprs`), so Rename changes only the name and every use keeps working; an assembly's new
  variable gets its id from its name (no id editing in assemblies, as live); older assemblies' random `in…` ids
  become their names on load (`Assembly::upgrade_config_variable_ids`). e2e renaming_a_configuration_variable_keeps_its_uses.

## From qa (2026-11-09 10:45 UTC, lead's bisect): asm-robot-relations / asm-scissor-lift dropped from the corpus refetch, not code

The drop core saw (robot-relations 139 -> 137 occurrences placed, scissor-lift 192 -> 191) is **live's REST refetch of the 22
studios for your REQUESTS 2381 (05:51 UTC 11-09)**, not a commit: main 35fd1ddf (the 04:00 suite run's green, 139 / 192 there)
gives 137 / 191 on the refetched corpus and 139 / 192 again with live's backed-up UI-method files put back
(/root/live-scratch/q2383/backup/<doc>__<element>.json; release, vibe4, qa-scratch/bisect/asm-35fd-oldcorpus.log and
asm-35fd1ddff.log). git bisect over 35fd1ddf..680dcece7 found every commit "bad" for the same reason. With the queries back,
the translation now goes wrong where it had been skipping them:
- robot-relations, studio a4e419: Extrude 2's booleanScope translates to `SWEPT_BODY: feature FWHIDTtUzrQS8Ho_0 was not built`,
  Fillet 1's entities to `SWEPT_EDGE: its curves share no vertex`; two occurrences (MGvcieiG00RPKCX0P, MJxUdRLLCX/VmYts4) are
  then "not inserted", and "Fixed Side <1>" / "<3>" find no part `Fixed Side` (the studio's parts come out as Part 1 / 2 / 3);
  Revolute 1 and 5 lose their connectors. Mates 9/11 -> 7/9.
- scissor-lift: 192 -> 191 occurrences, mates 10/17 -> 12/12 (better), studio 264eb3's Move face 1 / Mirror 1 / Linear pattern 1
  errors gone.
Repro: a folder with just the two (vibe4 ~/qa-bisect/docs: current corpus; ~/qa-bisect/docs-old: the backups in place) as
NOSHAPE_PARITY_ASSEMBLIES, `cargo test --release -p noshape-ops --test all parity::parity_assemblies -- --ignored --nocapture`.

## From psui (2026-11-09 13:59 UTC, lead: live2 2412): the pin-block connectors, settled; the flip icon is not
- **Connectors: ours already matches live.** live2 2412: Revolute 1 over REST has no primaryAxisAlignment, both
  implicit connectors Z = (0, 0, −1) in world (Part 1's hole at (0, 0, 10), Part 2's pin at its own (60, 0, 15),
  moved by (−60, 0, −5)); corpus asm-flip-midaxis-default / -flipped say the same. Ours (ui_parity start, ops
  `assembly_connectors`): pin mid axis (60, 0, 15) Z −Z, hole mid axis (0, 0, 10) Z −Z, the pin unrotated at
  (−60, 0, −5). A probe of all 19 corpus documents with axis connectors (ours' implicit connector Z vs live's
  `mateConnectorCS.zAxis`, part coordinates; patch /root/psui-scratch/q2412/cz-probe2.patch, env
  NOSHAPE_PARITY_CONNECTOR_Z): of 172 connectors, MID / TOP / BOTTOM_AXIS_POINT agree wherever ours has the same
  point, except mid axes on faces in isaac-ball (4) and pattern-std (2), live +Z / ours −Z at the same point, and
  pinslot (2, 0.34 mm apart): worth a look, not pin-block. 14 found no point of ours (hand-drill-pinslot 8,
  infento-subs 2, …). Nothing in `implicit.rs` changed.
- **What differs is the flip icon when editing** (asm-mate-edit-escape, asm-mate-menu-edit): live shows the plain
  `flip-direction` for the REST-made Revolute 1, ours `-opposite` (MateDialog::flip_icon_on: `-opposite` while the
  model's flag says the axes agree). New mates after both picks show `-opposite` in live for agreeing connectors
  (asm-fastened-two-faces, nothing turned), which ours matches. So live's icon seems to show the stored
  primaryAxisAlignment (set by the UI's pick, absent over REST), which ours' one flag can't tell apart. Asked live2
  (REQUESTS 14:00) for a UI-made mate's stored value and its icon when edited. run.py's icon rule is right as it is
  (inverting it for mate dialogs breaks 12 asm-* scenarios).
- End points (probe on all 13 corpus documents with TOP / BOTTOM_AXIS_POINT): live's TOP Z points out of its end,
  BOTTOM Z into it (both run bottom → top), and ours agrees at all 23 points it has (2 more found no point of ours). Ours' bottom → top follows
  OCCT's axis: pin-block's hole has ours' "top_axis" at z 0 (Z −Z); live2 calls the z 20 opening its top, which
  would make it +Z there: the one case where ours would differ, and what the UI case's auto-flip suggests. The
  14:00 capture settles which end live calls top on a hole.

## From psui (2026-11-09 08:55 UTC, lead): the pin-block Revolute flip, from the code
- Our axis connectors on a cylinder (model-core `regen/implicit.rs`) take Z from OCCT's cylinder surface axis as
  is: no face orientation, same for Mid / Top / Bottom axis. pin-block's hole is the inner loop of Sketch 1's region,
  so its circle (and the extruded hole's axis) runs −Z; the pin (an outer loop) runs +Z.
- Live: in the UI case (asm-revolute-pin-hole-flip, live2 2241, 11-08 03:27) the hole's **top** axis point has Z down
  (against the pin's) and the mate flips itself: ours matches that today. But Revolute 1 in pin-block (made over REST,
  **mid** axis points, flip off) keeps the pin upright: either live's mid-axis connector on the hole points up (unlike
  its top point) or the REST mate's connectors differ. Changing our hole axis would break the UI case, so nothing is
  changed until live2 reads Revolute 1's two connector axes (REQUESTS 08:50). If live's hole mid axis is +Z and its
  top axis −Z, the rule is per inference, in `implicit_connector`.

## From psui (2026-11-09 08:51 UTC, lead: psui holds the assembly interaction area while asm is paused)
What psui took and changed, so you pick up cleanly on Monday (all asm-*, vibe3 runs; logs psui's
/root/psui-scratch/cmp-asm3.txt):
- **Match:** 133 → 139 of the 142 recorded asm-* scenarios (612 states: 602 → 609). Fixed:
  - Change configuration… reads as live's panel (document, studio, each shown input and value, parts, sketches:
    `config_panel_text`), and Esc closes it from the page (`on_cancel`).
  - Change to version… on this document's own parts: the Reference manager lists the document with its Start
    ("Main ⇨ Start"), opens on Selective update, counts "Newer versions available for 0 documents" (its own row isn't
    newer); the state reads it as live's panel. With a linked newer version it still opens on Update to latest.
  - Create empty subassembly takes no undo step (store `NOT_UNDOABLE_PREFIXES`; e2e assembly_instance_menu).
  - Editing a mate: its row shows selected and related, the Mate features folder related, a double click's own
    selection goes (live asm-mate-menu-edit / -edit-escape), without relating the instances.
  - The graphics right-click takes the face under the click itself (View normal to was missing when the menu came
    before the hover pick, asm-section-view-edit).
  - Triad Align / Anti-align with Z: the axis goes to ±Z and the next axis to +X (live's X arrow: X → Z, Y → X,
    Z → Y), not the shortest turn. Only the X arrow is recorded; asm-triad-z-anti-align-with-z asks for one more.
  - run.py `runner_names`: a panel's text naming live2's document / its "start <name>" Part Studio reads as ours'.
- **Left (3):** asm-mate-edit-escape / asm-mate-menu-edit: the edited Revolute's flip toggle. Ours builds pin-block's
  Revolute 1 flipped (default flip keeps Part 2's orientation) with the same placement as live's unflipped one, so
  one of our mid-axis connectors points the other way: yours (implicit connector axes); live2 asked for the mate's
  primaryAxisAlignment and both connectors' Z (REQUESTS 08:50). asm-menu-switch-to-part-studio: live's take missed
  the click (re-record asked).
- **Coverage:** menu items 80/89 → 89/89 with 10 new scenarios (dynamic suppression Edit / Remove expression, Create
  new context, Open linked document, Show all instances, the Insert dialog's Set to last viewed / Reset to default,
  a pattern field's Configure, Edit standard content instance…, triad Z anti-align); all run on ours, filed with live2
  (REQUESTS 08:50). New marks: Insert dialog tabs (`tab:<label>`), standard content `button:Insert`, the configured
  row's ▾ `caret:Insert configuration:<studio>`, the pattern and Mate connector dialogs' number fields
  (`field:<label>`).

## From psui (2026-11-09 03:07 UTC, lead: for when you're back): the 8 asm-* scenarios that don't match in a full run
psui's full ui-parity run on 4be31cf6's tree (vibe5, 1358 scenarios; the same with and without its checkbox change):
asm-mate-edit-escape / asm-mate-menu-edit (a mate dialog's sixth field `checked` live true, ours false),
asm-menu-change-configuration (live reads a "Change configuration" panel), asm-menu-change-to-version (dialog kind
live "panel", ours "other"), asm-menu-create-empty-subassembly (Undo on in ours after it, off in live),
asm-menu-switch-to-part-studio (the instance menu's rows), asm-triad-align-with-z / -anti-align-with-z (instance 1's
transform[1]: live 1, ours 0). Logs: psui's /root/psui-scratch/cbreg/new.log (first differences).

## From studios (2026-11-08 20:04 UTC, lead's call): ☰ › Print… is studios' in the Assembly
The Print setup panel, its dashed frame and Download image / Print live in app-core `print_setup.rs` (geometry in
`noshape_scene::print`, op `render_print`). The Assembly only hooks it: `AssemblyView::open_print`, `on_print_action`, a `print`
field, the panel and frame drawn in `render_left_layer`, and Esc (Cancel) closing the panel first. Please leave those hooks to
studios; ask if you need them moved.

## From core (11-08 14:14 UTC): suspension's Knuckle 6/28; Extrude 4's regions are degenerate on our side
- EXTRUDE-UP-TO-FACE-OFFSET fixed (4b09e98c); with your 8c0f052e2 the Knuckle runs 1 -> 6/28 (Extrude 2 as live).
- Next difference, Extrude 4 (Remove 10 mm of 2 Sketch 2 regions): live removes exactly the lenses JOO + JOS
  (2 × 1,501.62 mm² × 10 = 30,032 mm³: 395,393.5 -> 365,361.1). Ours resolves the regions to
  `region:Fcb86c6ca2bb3:0,12,13,20,21,28,29` and `…:0,36,37,44,45,52,53`, whose prisms are degenerate (each
  invalid, volume ~3.5e-16 m³, 16 faces, y ±30 / z 63-110 mm where live's lens is y ±39.7 / z 38-110 mm), and
  OCCT's cut of such tools returns an invalid part of 396,228 mm³. The run notes "Sketch 2: 12 PROJECTED (not used:
  its model geometry not found)": our Sketch 2 lacks its projected curves, so its regions aren't live's and the
  area mapping lands on these. Case dir: vibe5 ~/core-cases/susp-knuckle; the cut: ~/core-cases/knbool.
  Yours (the translator's projected geometry / region mapping); tell me if it turns out to be a sketch-solver or
  region-builder bug and I'll take it.

## From dx (11-08 09:29 UTC): XT-B3-BINARY-TRANSMIT landed (fe7b499c)
The Parasolid reader takes the bare binary (`B`, little-endian: Cadenas PARTsolutions' vendor parts) and version 9's
schema (SCH_900000_9008). asm-xdev-subs' 34 bare files and its v9 text file read now: 32 valid solids, 3 multi-solid
parts (a7bd858292…, 35719e9b95…, 2ee17582b9…, 4-10 solids each) that OCCT's check flags; please rerun xdev-subs and
tell me if those three differ from live.

## Paused (2026-11-08 20:58 UTC; lead: Claude quota, restart fresh Monday 11-10)
Where it stands:
- Everything of asm's is on main (last: f22e34ec, the part-name rule) but 541ff4f0 (NOSHAPE_PARITY_TRACE prints the
  live part → our part mapping), queued 20:42: check `tools/tiers/land.sh --status` first; a failure there is the first
  task. Nothing uncommitted.
- Assembly corpus, corpus15 (main 17:40, full run, 109 min; SCORE row): 1240/1701 occurrences, 155/345 mates. Since
  then welding-config mates 12 → 20/26 (part-name rule), not in a full run yet. Full runs need `timeout 10000` (the
  sketch bodies made translation slower: real work, sketches now map their references).
- ui-parity asm 131/141 (09:3x). Left, with their reasons, in "Latest (09:3x)" below.
- Speed: an assembly evaluation builds its studios' source options once (b142ef36); a full corpus run is the check.

Next items, in order:
1. Unresolved mates (corpus15: 78 translate but don't resolve). welding-config 13 done. hand-drill-pinslot's 15 are all
   on its Chuck to Gearbox Shaft (studio 7ed710): Fill 1's and External thread 1's queries are empty in live's fetch:
   waits on REQUESTS 2381 (204 empty queries in 22 studios). infento-subs' 26: run it alone with
   NOSHAPE_PARITY_TRACE=1 (~65 min on vibe3; dir ~/agents/asm/parity-inf) and look at which live part each failing
   mate's instance became (`live part … -> ours …` lines): so far every case was a wrong part mapping or a part whose
   geometry differs from live.
2. Then the 516 untranslated mates: connector origins on an edge (227) or face (57) we don't have: rank by studio,
   most are studio-geometry gaps (dx / core tickets) or wait on live (2271 Feature Studios and files, 2381).
3. The now-active mates that pull parts ~1e-4 rad off live (pattern-std's frame parts, infento-subs): one frame part's
   connector is off; compare its face normals with live's per-feature lines.
4. ui-parity: 2241 (1) mate flip, 2296 Align with Z, 2269 toast fade: re-run those scenarios when live2 answers.
- Waiting on others: core EXTRUDE-UP-TO-FACE-OFFSET (suspension's Knuckle Extrude 2; then time its Circular pattern on
  the real Knuckle), live 2271 / 2381, live2 2241 / 2269 / 2296.
- Scratch: /root/asm-scratch (bin/: rank_mates.py, rank_studios.py, ps_compare.py, clamptest.sh); on vibe3
  ~/agents/asm/{target, target-app} (two target dirs: corpus runs in target, single-document runs in target-app),
  model dirs ps-knuckle / ps-shaft for running one studio alone with parity_models.

## Latest (2026-11-08 19:4x UTC)
- corpus15 (full, 109 min): 1240/1701 occurrences, mates 155/345 (SCORE). Unmatched mates by cause
  (/root/asm-scratch/bin/rank_mates.py): 516 not translated (connector origins: 227 on an edge of ours that isn't
  there, 57 on a face: studio geometry), 96 overdefined, 78 connectors that don't resolve (infento-subs 26,
  hand-drill-pinslot 15, welding-config 13), 15 inconsistent.
- Fixed today: an assembly regenerates a studio with its sources (a Frame's other-studio profile; ops' assemblies had
  none at all: Derived and custom features too), made once per evaluation (P0 speed: per studio it cloned the document
  each time); parts map by volume first, then creator (clamp's Revolute 17: Pin and Spacer were one part). Revolute 17
  was no morning regression: every main commit from 56603985 fails it the same way; corpus8's pass came from
  uncommitted work.
- Harness time: the translator reads each sketch once per step; with the sketch files the full run is ~110 min (run
  it with `timeout 10000`).
- Next: the 78 unresolved connectors (same studio, translator vs assembly regeneration?), then the edge-origin
  connectors by studio.

## Latest (2026-11-08 12:2x UTC)
- live 2267 answered: 211 studios have `studios/<element>.sketches.json` (every sketch's bodies at the final state).
  The translator reads them (`refs_from_sketches`, both harnesses: the assembly suite and a model dir's
  `*sketches.json`): hand-drill-pinslot 27 → 38/41, isaac-ball 73 → 79/121 (alone). A query of sketch regions all in
  that file maps by their areas when its text's regions don't have live's total area (a region query's text names all
  the sketch's regions; suspension's Extrude 1 ring, whose text names the disc). Only the file's regions: a per-feature
  line's may be pieces a later feature imprinted (exercise-chino went 9 → 6 with them, 9/9 again).
- Suspension measured for the first time (main with core's pattern fix d999cf37): 25/30 occurrences, 2/5 mates, the whole assembly in
  35 s (it never finished; the Knuckle studio alone 554 ms, live 214). The Knuckle run alone (model dir on vibe3 ~/agents/asm/ps-knuckle): Extrude 1 matches live
  now; Extrude 2 is the first difference (ticket EXTRUDE-UP-TO-FACE-OFFSET, core); its Circular pattern then fails at
  once (core's "close round no volume" check) as its faces aren't live's.
- corpus12 (main 10:00, full): 1216/1701, mates 112/325 (xdev-subs 200/286 with dx's reader). clamp-pattern's Revolute
  17 stopped resolving between corpus8 and corpus11 (one mate, its connectors translate the same; not found yet).

## Latest (2026-11-08 09:3x UTC)
- ui-parity asm 131/141. The graphics right-click menu opens where live's does (probed in live: top-left 2 px under
  the pointer's pixel; above it with its bottom 4 px under; the row under the pointer hovered only when the pointer is
  between pixels, as Chrome hit-tests while reporting whole pixels; the driver notes a target's exact point, the
  triad's at the last face pick's, an empty spot's at a whole pixel). Input propagation with nothing to propagate shows
  live's info line (no table) and is recorded; psui recorded Visibility conditions. Left: mate flip (2241 (1)),
  Align with Z's rotation (REQUESTS 2296), dynamic-suppression toast (2269), Reference manager / Change configuration
  (live's lane document's names), Switch to Part Studio (start naming), Release… (live's upsell), Create empty
  subassembly (live's shortcoming).
- Frames with another studio's profile (psui e1d80331): the translator writes `<tab>#<sketch>.4294967295`, the source
  studio built first (profile-only studios too): pattern-std 116 → 137/162.

## Now (2026-11-08 06:14 UTC)
- Landed b16fb21f: Hide / Show mates with nothing to change is an undo step (live2 2241 (2)), a click on a submenu row
  opens it, ticket FACE-PATTERN-30-CUT-SLOW (core: suspension's one Circular pattern takes ~20 min in OCCT).
- This land: one AssemblyEvaluator per document (subassembly tabs share the studio cache); translator: volume bands
  from live's per-feature lines, an Import of a native assembly zip builds in its Part Studio, standard content live
  didn't fetch inserted from our catalogue by part name (`standard_spec_by_part_name`); `find_occurrence` takes a name
  with a slash as one instance (inch standard content "1/4-20" couldn't be addressed); a new List configuration input
  opens its "Name" row for typing (live2 2208 (1)).
- Assembly corpus: corpus8 913/1701 occurrences on 23 documents (856 on the 20 measured before, was 827), mates
  107/286; corpus9 (std + import, 5 documents): bjr-config 102 → 119, robot-relations 100 → 121, pinslot 19 → 25,
  corexy 24 → 26 (xdev-subs 7/286: its subassemblies). robot-relations' mates 2/5 → 0/0 in corpus9 was the slash
  bug (fixed since, not re-measured). Part Studio suite after: 70/86, 724/934, no document worse.
- ui-parity asm: 119/141 (01:3x, before the config fix). Left: config focus after the first step (messages "Configuration
  copied." / "Inputs copied.", modal focus of Input propagation / Visibility conditions / Release), the right-click
  menu's highlighted last row (6 scenarios: live's pointer lands in the "Delete" row of a menu opened above it; ours on
  its left border: live's menu geometry not recorded), mate flip (2241 (1) not recorded), dynamic suppression toast
  (REQUESTS 2269), Reference manager (live's lane document's version "Start"), Switch to Part Studio (live's studio is
  "start two-blocks", ours "Part Studio 1").
- Asked: 2267 (sketch bodies for the corpus' studios; 3 studios without any live lines), 2269 (toast fade), 2271
  (Feature Studios and imported files not fetched). Ticket FRAME-PROFILE-OTHER-STUDIO (psui, 41 occurrences).
- Then (06:45): a subassembly live left out of its fetch (suppressed instances) no longer stops the others being built:
  xdev-subs 7 → 90/286, robot-relations 123/139 (mates 2/7). Corpus total ≈ 1044/1701. xdev-subs' rest: 34 vendor
  Parasolid files our reader refuses (ticket XT-B3-BINARY-TRANSMIT, dx).
- ui-parity asm 125/141 (with the Mate connector dialog's picks: selected, an undo step of the dialog, focus none after a pick): Copy input / Copy all inputs copy to the clipboard with "<name> copied." (live: no
  edit; coverage now config_copy_inputs), the toast read after the panel's info line, Esc dismisses it; runner
  `config_files` seeds live's remembered Export (asm-menu-export). Left: the two shared modals (Input propagation,
  Visibility conditions) aren't recorded and live's title carries its lane copy's name; menu highlight; mate flip;
  Reference manager; Switch to Part Studio (live's studio name); Create empty subassembly undo (live's shortcoming).
- Next: the next gaps by occurrences (rank_studios.py on the newest log); most of what's left waits on live (2267,
  2271) or other areas (XT-B3-BINARY-TRANSMIT, FRAME-PROFILE-OTHER-STUDIO, FACE-PATTERN-30-CUT-SLOW).

## Paused (2026-11-07 20:08 UTC; lead: Claude quota, resume Monday 11-10)
Where everything stood:
- Landed 11-07 20:23 UTC as 9cd86358: the batch below (translator for the corpus, open-composite mesh fix,
  same-connector pick, Edit dynamic suppression submenu, pin-block-apart start + asm-revolute-pin-hole-flip
  scenario). Nothing of asm's is queued or uncommitted.
- Assembly corpus (corpus7, SCORE.md row 11-07 20:08): 827/1291 occurrences on the 20 documents measured, mates
  90/182. Not measured: **suspension ran 35 min without finishing** (stopped; the next slow case: trace it with
  NOSHAPE_PARITY_TRACE=1 on a copy in parity-one, profile with gdb on vibe5, which allows ptrace of a child: see
  /root/asm-scratch/bin/prof5.sh), then tvc-mount, welding-config, xdev-subs. Down since corpus4: hand-drill-pinslot
  18 → 12/41 and isaac-ball 73 → 71/121: diff their messages (corpus4.log vs corpus7.log in /root/asm-scratch) first.
- Part Studio suite with all of it: no document worse (ps-after5: 61/88, 648/947).
- Pin-block scenarios after dx's fixture fix: 36/41 (before this batch). Left: asm-mate-menu-edit and
  asm-mate-edit-escape (our hole axis connector Z may point the other way from live's: waiting on live2 for
  asm-revolute-pin-hole-flip, REQUESTS 2241), asm-menu-hide-mates / asm-menu-show-mates (live2 2241: Hide mates is an
  undo step even when nothing changes; ours drops no-op commits in crates/store's UndoStack::commit: needs a way to
  record a no-change step). Re-run the 41 (/root/asm-scratch/pinblock-set.txt) and asm-menu-edit-dynamic-suppression
  once the batch is on main.
- Waiting on live / live2: 2241 (1) the pin-hole flip capture; the config panel focus input and Export format reset
  (2208); A1000 timed in live (2189).

## Latest (2026-11-07 18:24 UTC)
- Translator for the assembly corpus (lead 11-07; one line each in dx.md / skasm.md): standard-content configurations,
  part ids by creation counter (`onshape_counter`, `part_by_creator`), id-only regions by live's part volumes or the
  outer region, revolve axes on the profile's side, Imports from live's foreign.json, and live2's per-studio
  `.live.jsonl` as the translator's refs. NOSHAPE_PARITY_TRACE=1 prints each studio feature / insert / mate with its
  time (the suite hung in robot-relations).
- P0 speed (fixed): an instance of an **open composite part** was meshed at Very fine on every assembly evaluation,
  and a mesh that ran into the mesher's time limit wasn't kept: robot-relations' composite of an imported part cost
  ~60 s per insert. Now Auto unless "Use best available tessellation", kept once made (T1
  an_open_composite_instance_is_meshed_once_at_the_display_quality).
- Pin-block re-run after dx's start fix (f3fec6a8): 36/41 scenarios. Fixed: the same connector picked twice adds
  nothing (asm-cylindrical-pin-hole). Asked live2 (REQUESTS 18:06): a pin-in-hole mate's automatic flip (our hole
  mid-axis Z may point the other way: asm-mate-menu-edit, asm-mate-edit-escape) and whether Hide / Show mates with
  nothing to change is an undo step (asm-menu-hide-mates, asm-menu-show-mates; our store drops no-op commits).
- Edit dynamic suppression is a submenu (Edit expression…, Remove expression) as live2 recorded.

## Earlier today (2026-11-07 08:40 UTC)
- ASM-CONFIGURE-PATTERNS done (live 2209): patterns and mate connectors configurable (rows, dialog fields, configured
  counts making that configuration's copies, configured edits). Every P1 of the audit is now closed but the rest of
  ASM-API-GAPS (standard content as a replacement, explode-line entities, mate value typing / reorder, a connector's
  move from the mate dialog).
- Model suite: the corpus' part names now map to ours by name, by the only part, or by live's volume band
  (`.mass.json`): bjr-config 4 → 46/137, clamp 0 → 9/27, gearbox 0 → 11/23, hand-drill 0 → 13/41. What's left is
  mostly Part Studio translation (dx's translator: variables, patterns, external references).
- Waiting on live2: config panel focus (which input), the Export format reset, asm-linear-pattern-third-direction,
  what Edit dynamic suppression shows.

## Before (2026-11-07 08:00 UTC)
- Since 05:01: every P1 but ASM-CONFIGURE-PATTERNS (waiting on live2) and the rest of ASM-API-GAPS is fixed:
  relation directions (B = −ratio·A), linear pattern Second/Third direction, tapped-hole auto-size, BOM of linked
  parts, configuration variables, flexible subassemblies with relations. New ops: assembly_properties,
  assembly_measure, assembly_mass_properties, assembly_update_references; assembly_replace takes several instances,
  sketches and other documents.
- ui-parity: 107/140 at 06:20; the triad ring axis, Dynamic suppression panel (two-stage Enter, message, selection),
  Reset / unlock messages, folders "(n)" since.
- Model suite: live's corpus arrived (24 assemblies): 4/24, occurrences 181/1731, mates 19/23. Most failures are Part
  Studio translation gaps in big public documents (their instances "not inserted"); next to dig into.

## Earlier (2026-11-07 05:01 UTC)
- Round 4 (lead): the three P0s (BOM ×2, mass overrides) and the face-click P1 are done. P1s fixed since:
  ASM-DISPLAY-STATE-NESTED, ASM-REPLICATE-SEEDS, ASM-CONFIG-VARIABLE-NOOP, ASM-BOM-EXTERNAL, ASM-FLEX-SUB-RIGID
  (relations only: patterns / tangent mates / nested flexible subassemblies still make a subassembly rigid).
- Open P1s: ASM-CONFIGURE-PATTERNS (asked live2, REQUESTS 04:50), the rest of ASM-API-GAPS, the three live checks
  (ASM-STD-AUTOSIZE-HOLE, ASM-LINEAR-PATTERN-SECOND, ASM-RELATION-DIRECTION).
- ui-parity: 90/129 at the 04:20 run (82 before); fixes since (named positions, Export modal, Display states,
  Replace instance, Tangent / Width, Animate, Driving mate selector, Group picks) take it higher.
- Left on purpose (live's shortcomings, not parity): Create empty subassembly not undoable in live; Hide mates on
  already hidden mates records a no-op step in live; Release… opens live's Professional-trial upsell; Change to
  version… shows live's document names. Waiting on live2: config panel focus (which input), Export format (live's
  account remembers STEP), "New folder (1)", the 5 subassembly re-records.

## State (2026-11-06 20:40 UTC)

### Open P0 / P1 assembly items
- None open. TICKETS: the only open assembly ticket is P2 UIP-ASM-FASTENED-FLAKY (skasm fixed a fast pointer move
  ending the undo pause, 11-06 17:00; psui to re-run asm-fastened-undo-picks on vibe5).
- Speed, re-measured by asm 11-06 22:40 on vibe3 at f2bcbb96 (perf.sh, release, 3 runs; the range is the 3 runs):
  | | live (its A200, 162 mates; live 1741, software GL) | a200-live (196 inst., 162 mates) | a1000-live (1009 inst., 900 mates) | a1000 (2380 mates) |
  |---|---|---|---|---|
  | open (load + first evaluation + scene) | list 12–15 s, graphics 22–25 s | 38–40 ms | 131–153 ms | 123–178 ms |
  | regenerate (pin depth edited) | 12.9 s view, 15.6 s stable | 20 ms | 100–120 ms | 112–136 ms |
  | add a mate | 1.9 s answer, 7.5 s view | 17–22 ms | 28–49 ms | 43–61 ms |
  | drag frame (solve + scene) | one update every 2.3–2.5 s | 0.8 ms (1200 fps) | 1.8–2.9 ms (350–570 fps) | 3.4–4.4 ms (230–290 fps) |
  | insert a pin | 5.0 s | 8–9 ms | 13–20 ms | 18–30 ms |
  In the app (this machine, load ~2.4, llvmpipe as live's VM, screenshots every ~0.1 s): a1000-live, Assembly tab
  clicked → graphics drawn 0.68–1.18 s, settled 1.0–1.9 s (3 runs; the library part is ~0.15 s: the rest is the
  first frames on software GL, not measured further). Live never timed A1000 (≈ 2800 REST calls to build).
- Speed (PLAN 2.7, skasm 11-03 on vibe3): every target met. 200 instances / 300 mates: drag frame 0.29 ms (off plane
  1.15; target 8.33), insert 1.4 ms (target 20), add a mate 3.5 ms. a1000 (2380 mates): drag 4.2 ms, insert 22.4 ms
  (a whole evaluation). Live: drag ≈ 1.2 s a step, insert 1.3–2.2 s.

### ui-parity
- Live recordings: 7 asm-* (all matched on skasm's last run, asm-fastened-undo-picks flaky 1 in 4).
- Filed with live, not recorded yet: batch 1 (25, REQUESTS 2156), batch 2 (20 menus, 2158), batch 3 (40, below;
  filed 11-06 22:1x, land 056b6606 queued). On our side every batch 3 step runs (nothing "not shown").
  Waiting: whether live's recorder clicks the right-hand panels (exploded views, named positions, BOM).
- Batch 3 (asm 11-06, 40 scenarios): the assembly row's menu (Unfix all, Collapse / Expand all, Check
  interference…, Rename), the graphics menus (a face: Section view… with Turn off / Edit section view…, Select
  other…, View normal to, Find in instance list, Clear selection; empty: Isometric, Clear selection), the instance
  row menu's Export…, Release…, Exit isolate, Create empty subassembly, Add selection to folder…, Unpack folder,
  Hide / Show mates, Edit in context, a mate connector row's menu (Hide other / Hide all / Show all mate connectors),
  the mate menu's Hide all / Show all / Hide other mates, the four relation tools, Display states, Create Part Studio
  in context, Show mates mode (tool and h), Update all references.
  New targets: the assembly's own row, instance folders and "Mate features" are `{"instance": …}` rows (live's runner
  already finds rows by text).
- Found by batch 3 on our side before live's recordings (live's rules from behaviours.md):
  - Enter with the focus in the graphics now accepts the open assembly dialog (Mate connector, Group, mate, patterns,
    relations, Replace, folder name, Section view), as its ✓ (behaviours "Enter accepts the open dialog", line ~1200;
    skasm's open question). Binding `enter` → Accept in the Assembly context; T1 assembly-ui
    `enter_in_the_graphics_accepts_the_open_dialog`.
  - Connector, relation and group rows are `mate:` targets; the Section view dialog's ✓ / ✕ are `button:ok` /
    `button:cancel` (app-core section_dialog, the Part Studio's too). Live's first folder is "New folder" as ours.
  - `{"view_cube": …}` targets work in an assembly (app-core `view_cube::ui_cell_point`, shared with the Part Studio).
  - Not differences: a part instance's row menu has no Section view… / Check interference… (live 1259 / 1357: graphics
    menu and the assembly row only); mates start hidden in live too, so a mate row's menu offers Show / Show all mates
    until `j`.
  - Not in the recorded state yet (wait for live's format): Display states, Create Part Studio in context, Check
    interference, Section view, Add selection to folder's name dialog.

- Batch 4 (asm 11-06 22:4x, 6 scenarios, REQUESTS after batch 3): Dynamic suppression ▸ Add expression… / Edit dynamic
  suppression, a new subassembly's Dissolve / Expand selected / Collapse selected (ours names it "Assembly 1 <1>"),
  the toolbar's Customize toolbar…. Ours: an expanded subassembly's rows are now in the recorded state (as
  `push_children` draws them), and the Insert tool is also `tool:Insert` (live's runner takes a tool's text; batch
  1's Insert scenarios would not have found it). Asked of live: a triad target and configurations in starts (the last
  uncovered menu items: triad Move to origin / Align / Rotate, the configuration items).

- Batch 5 (asm 11-06 23:50, 31 scenarios): the right-hand panels (Exploded views: add, row menus, Show / Hide all
  explode lines, Duplicate, Reset starting position; Named positions: add, Apply / Edit / Duplicate / Update / Move up
  / Move down; BOM: Copy table), the assembly's Configuration panel (Add input, Copy input / all inputs, Visibility
  conditions, Input propagation, Configure suppression), a subassembly's Unlock (locked first) and Add to named
  positions, Change to version…, Edit named position driver mate…, Switch to Part Studio. New targets: the strip's and
  panels' buttons by tooltip, `panel_row`, `panel_more`; the assembly's config panel marks psui's `config_more`.
- Left uncovered (13 menu items): the triad menu (Move to origin, Align / Anti-align with Z, Rotate 90 / 180 deg) and a
  configured Part Studio's instance items (Change configuration…, Select instances with same configuration ×2, the
  Insert dialog's Reset to default / Set to last viewed): both asked of live (batch 4); Edit in context ▸ Create new
  context (needs a feature picking ghost geometry first, as live), Open linked document (another document), Edit
  standard content instance… (needs standard content inserted).

- 11-07 (lead: settle batches 1-5 from live's captures): every menu our runs record compared with behaviours.md's
  captures (instance row 1357, viewport instance, top row 1222 / 11150, Origin row, mate row 1361, connector row
  29018, folder row 1224 + screenshot assembly/537 (its Dynamic suppression ▸ / Expand ▸ / Collapse ▸ are live's: the
  text list left them out), subassembly row 1226, explode rows 3139 / 1847, named position rows 1345, BOM ⋯ 1280,
  assembly configuration ⋯ 1298, panel ⋯ 28421, configured instance 6116): all equal. Dialogs settled by the
  captures and now in our recorded state: Origin of new Part Studio, Dynamic suppression, Driving mate selector,
  Reference manager, Section view (app-core `section_dialog::state`, shared with the Part Studio: its 6 section
  scenarios still match), Properties / Instance properties modals (focus "other"). Not captured, left for live2's
  recordings: Check interference, Display states, Animate, the Folder name dialog.
- Batch 6 (11-07, 11 scenarios): the triad's menus (`{"triad": "x" | "ring-z" | "origin"}`) and a configured Part
  Studio's instance items (start `config-blocks`: starts.json `"configuration"`, built by ops ui_parity). Menus
  81/86; left: Create new context, Open linked document, Edit standard content instance…, the Insert dialog's Reset
  to default / Set to last viewed (each needs a longer setup).

- 11-07 live2's 98 assembly recordings (batches 1-5) landed: 23/98 at first, **68/98 (351/409 states)** after:
  runner normalizations (live's start Part Studio "start <name>", its lane copies' " Copy N", a new subassembly's
  "Assembly <n>"; prefix targets `{"instance": "Assembly"}` as live's findXY), our recorded state (the focused field of
  a dialog with ✓, a rename / Dynamic suppression / Folder name / Display states input, a modal's focus; pending rows
  of a new relation / connector / group / pattern, selected, editing, red until complete; panels Dynamic suppression,
  Folder name, Select other; Isolate / Make transparent, Interference detection, Animate mate; no Insert dialog; a
  selected row isn't "related"; empty required pattern fields red; a suppressed mate's REST status OK) and behaviour
  (Enter-accept, menus of a hidden / suppressed / isolated instance, empty space with a selection gives its menu,
  Show all instances, Collapse all folds Mate features, Esc closes Display states, no selection after Move to new /
  Create empty / Dissolve subassembly, no snap-mode message, the connector at an instance origin keeps its field
  active, a mate opened for edit keeps "Mate connectors" active, Check interference from the assembly row holds the
  assembly). Asked of live2: re-record the five subassembly scenarios; "New folder (1)".
  Left (main causes): a face click selects the face in live (FACE) and the instance in ours; a right-click selects
  the row in live (selection 1 after Add expression…); Select other's first row (the face) missing in ours; Create
  empty subassembly not undoable in live; Hide / Show mates undoable in live; Reset with no effect's message
  "Resetting mates had no effect."; Export / Release modals don't report themselves; a mate's flip toggle on an
  edited mate (start data); Tangent / Width dialog field counts.
- 11-07 feature audit (3 read-only auditors over Onshape's 44 assembly help pages): tickets in docs/TICKETS.md
  "Assemblies (asm)". Fixed: ASM-RELATION-TURNS (P0, the driver's whole turns read at each solve's start),
  ASM-RELATION-RATIO-VARIABLES (P0), ASM-DRAWING-VIEW-INDEX (P0: list edits re-point drawing views in one undo
  step), ASM-MATE-OFFSET-ROTATE-AXIS (P1). Next: ASM-BOM-CONFIGURED-SUBASSEMBLY, -PROPERTIES, ASM-MASS-OVERRIDES
  (verify), then the P1s.

## Assembly model suite (lead 11-06; docs/PARITY-SUITES.md §4, SCORE.md "Assemblies")
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over /root/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES; on
  vibe3 the copy is ~/agents/asm/parity-asm, rsync it after live adds documents). `rebuild_assembly`
  (crates/ops/src/parity.rs): Part Studios by dx's translator, then each level (subassemblies first, each its own
  "Sub <eid>" tab) inserted, fixed and placed as live's occurrences (a nested instance relative to its subassembly's
  occurrence), mates between the same connectors (root mates reach nested parts by path); top-level instances held at
  live's places, compared (top-level by placement, nested parts by assembly_list's `occurrences`), freed: our mates
  mustn't move them; mate status as live's. Not yet: patterns, groups, relations, mate limits, configurations, BOM
  (waiting on live's corpus to see their JSON).
- 11-06 23:54: 3/3 assemblies, occurrences 8/8, mates 5/5 (live's three built cases). Corpus of 15–20 public
  assemblies asked of live (REQUESTS, asm 23:5x).
- Agent API gaps found and fixed for it: a mate to a part inside a subassembly (assembly_mate / assembly_entities /
  assembly_connectors take "Sub <1> > Part 1 <1>"; e2e a_mate_reaches_a_part_inside_a_subassembly), assembly_list's
  `occurrences` (every part occurrence's place).

### Coverage (tools/ui-parity/coverage_assembly.py)
- 11-07 00:05 (asm, with batch 5): toolbar 31/31 (Mate relation = the relation tools), panels 3/3, menu items 73/86,
  shortcuts 5/5 (129 asm-* scenarios).
- 11-06 22:45 (asm, with batch 4): toolbar 30/31, menu items 51/86, panels 0/3, shortcuts 5/5 (98 asm-* scenarios).
- 11-06 20:50 (asm, with batch 3): toolbar 30/31, menu items 45/86 (92 scenarios).
- 11-06 evening (skasm): toolbar 23/31, menu items 18/86, panels 0/3, shortcuts 4/5.
