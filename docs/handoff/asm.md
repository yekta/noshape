# Handoff: asm (assemblies), from 2026-11-06

Taken over from skasm on 2026-11-06 (skasm keeps sketches and sheet metal). Area: crates/assembly, crates/assembly-ui,
the assembly ops in crates/ops and their e2e tests, the assembly (asm-*) scenarios of the ui-parity suite.

Worktree /root/projects/noshape-wt/asm (main, detached), target dir /root/target-asm, display :105, scratch
/root/asm-scratch (bin/: scenario generators). Builds and tests: `NOSHAPE_RB_HOST=vibe3 tools/tiers/rb.sh …` (vibe3,
8 cores; rb-hosts has no asm line yet, so without the variable rb.sh picks vibe4). The app here:
`NOSHAPE_RB_HOST=vibe3 tools/tiers/rb.sh --get noshape cargo build -p noshape`, then
`tools/ui-parity/run.py --app /root/target-asm/debug/noshape --display :105 <names>`.

Older assembly history: docs/handoff/skasm.md (assembly notes, speed numbers, fuzz) and assembly-behaviour.md.

## State (2026-11-06 20:40 UTC)

### Open P0 / P1 assembly items
- None open. TICKETS: the only open assembly ticket is P2 UIP-ASM-FASTENED-FLAKY (skasm fixed a fast pointer move
  ending the undo pause, 11-06 17:00; psui to re-run asm-fastened-undo-picks on vibe5).
- Speed (PLAN 2.7, skasm 11-03 on vibe3): every target met. 200 instances / 300 mates: drag frame 0.29 ms (off plane
  1.15; target 8.33), insert 1.4 ms (target 20), add a mate 3.5 ms. a1000 (2380 mates): drag 4.2 ms, insert 22.4 ms
  (a whole evaluation). Live: drag ≈ 1.2 s a step, insert 1.3–2.2 s.

### ui-parity
- Live recordings: 7 asm-* (all matched on skasm's last run, asm-fastened-undo-picks flaky 1 in 4).
- Filed with live, not recorded yet: batch 1 (25, REQUESTS 2156), batch 2 (20 menus, 2158), batch 3 (40, below).
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
  - `{"view_cube": …}` targets work in an assembly (app-core `view_cube::ui_cell_point`, shared with the Part Studio).
  - Not differences: a part instance's row menu has no Section view… / Check interference… (live 1259 / 1357: graphics
    menu and the assembly row only); mates start hidden in live too, so a mate row's menu offers Show / Show all mates
    until `j`.
  - Not in the recorded state yet (wait for live's format): Display states, Create Part Studio in context, Check
    interference, Section view, Add selection to folder's name dialog.

### Coverage (tools/ui-parity/coverage_assembly.py)
- 11-06 20:50 (asm, with batch 3): toolbar 30/31, menu items 45/86, panels 0/3, shortcuts 5/5 (92 asm-* scenarios).
- 11-06 evening (skasm): toolbar 23/31, menu items 18/86, panels 0/3, shortcuts 4/5.
