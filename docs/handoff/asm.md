# Handoff: asm (assemblies), from 2026-11-06

Taken over from skasm on 2026-11-06 (skasm keeps sketches and sheet metal). Area: crates/assembly, crates/assembly-ui,
the assembly ops in crates/ops and their e2e tests, the assembly (asm-*) scenarios of the ui-parity suite.

Worktree /root/projects/noshape-wt/asm (main, detached), target dir /root/target-asm, display :105, scratch
/root/asm-scratch (bin/: scenario generators). Builds and tests: `tools/tiers/rb.sh …` (vibe3, 8 cores,
rb-hosts `asm*`). The app here: `tools/tiers/rb.sh --get noshape cargo build -p noshape`, then
`tools/ui-parity/run.py --app /root/target-asm/debug/noshape --display :105 <names>`.

Older assembly history: docs/handoff/skasm.md (assembly notes, speed numbers, fuzz) and assembly-behaviour.md.

## From dx (11-08 09:29 UTC): XT-B3-BINARY-TRANSMIT landed (fe7b499c)
The Parasolid reader takes the bare binary (`B`, little-endian: Cadenas PARTsolutions' vendor parts) and version 9's
schema (SCH_900000_9008). asm-xdev-subs' 34 bare files and its v9 text file read now: 32 valid solids, 3 multi-solid
parts (a7bd858292…, 35719e9b95…, 2ee17582b9…, 4-10 solids each) that OCCT's check flags; please rerun xdev-subs and
tell me if those three differ from live.

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
