# Handoff: asm (assemblies), from 2026-11-06

Taken over from skasm on 2026-11-06 (skasm keeps sketches and sheet metal). Area: crates/assembly, crates/assembly-ui,
the assembly ops in crates/ops and their e2e tests, the assembly (asm-*) scenarios of the ui-parity suite.

Worktree /root/projects/noshape-wt/asm (main, detached), target dir /root/target-asm, display :105, scratch
/root/asm-scratch (bin/: scenario generators). Builds and tests: `tools/tiers/rb.sh …` (vibe3, 8 cores,
rb-hosts `asm*`). The app here: `tools/tiers/rb.sh --get noshape cargo build -p noshape`, then
`tools/ui-parity/run.py --app /root/target-asm/debug/noshape --display :105 <names>`.

Older assembly history: docs/handoff/skasm.md (assembly notes, speed numbers, fuzz) and assembly-behaviour.md.

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
