# Assembly feel coverage audit (psui, 2026-11-25)

The lead asked (11-25 12:3x) for the Part Studio audit's counterpart in the assembly: behaviours.md's assembly specs
(hover / select, triad and manipulators, mate dialogs, list rows, Insert) against the tests. Coverage counts a GPUI
test in crates/assembly-ui (`*tests*.rs`, 152 GPUI + unit tests before this audit) or a ui-parity scenario
(`tests/ui-parity/asm-*.json`, compared with live's state) that checks the rule.

Source: behaviours.md has 140 assembly sections with a feel word in their title
(/root/psui-scratch/asmcov/secs.py). The rules counted are the bold ones of the P0/P1 feel captures (11-17 to 11-22:
"assembly hover / select feel", "asm feel follow-ups", "manipulator drags, the gaps", "mate limits per type",
"Snap mode drag", "instance and mate list, Insert, mate connector dots", "list × viewport hover matrix",
"selection gestures", "assembly triad feel", "Insert dialog: Escape and ✓"). Imports, standard content, drawings and
the solver's own numbers are out of scope.

## Counts (rules tested / rules)

| Area | Before | After | Added (test) |
|---|---|---|---|
| Hover and select | 19 / 22 | 22 / 22 | a long press still selects at the release, Space clears, a drag from empty space moves the selection with no box (`a_long_press_selects_space_clears_and_a_drag_from_empty_space_moves_the_selection`) |
| Triad and manipulators | 11 / 12 | 12 / 12 | the plane square drags with no value box (`a_triad_plane_square_drags_with_no_value_box`) |
| Mate and mate connector dialogs | 11 / 13 | 12 / 13 | `a` / `q` in an open Mate connector dialog (`a_and_q_flip_and_turn_an_open_mate_connector`) |
| Instance and mate list rows | 9 / 11 | 11 / 11 | a row drags only past 8 px (`an_instance_row_drags_only_past_8_px`); rename: the name selected, Esc cancels, Enter / Tab / a click in the graphics commit (`a_rows_rename_selects_the_name_esc_cancels_and_enter_tab_or_a_click_commit`) |
| Insert | 10 / 11 | 11 / 11 | ✓ keeps the dropped and the pending instance, one undo takes both (`insert_ok_keeps_the_dropped_and_the_pending_and_one_undo_takes_both`) |
| **Total** | **60 / 69** | **68 / 69** | 7 tests |

**Failed, fixed (2):** in a list row's rename field, Tab did nothing (it reached the input as its IndentInline
action) and a click in the graphics left the field open (the field sits inside the view, so it never blurred); both
now commit as Enter does.

**Left:** "Dragging clamps at the limits" (a long drag on a Revolute with limits stops at 30° / 60°) is the solver's
drag; for asm / skasm.

## Already covered (the rules, one line each)

- Hover and select: mate flyout look and content; name-only instance tips; a hovered mate row lights both parts and
  its glyph; graphics → list lights nothing; connector reach; glyph reach and its click selects the mate; a part
  click selects the instance; window / crossing boxes, the crossing outline, Ctrl+box removes; a part inside a
  subassembly hovered and taken alone; a click / Shift+click on a selected face keeps it; Ctrl+click opens the menu;
  Esc keeps, an empty click clears; Select other reach; Shift+click row ranges; a hovered row outlines without a tint.
- Triad: the other handles fade, the dimension line / arc; 0.1 mm and 5° snaps, Shift frees them; the value box
  during and after the drag; Esc drops the box, the drag going on; the arrow keeps its grab point; the ball snaps
  over a face and stays on the point; no hover during a drag; the Z ring turns as live.
- Mates: inference dots only in a mate / connector tool, their 50 px square; Fastened jumps at once; A / Q on an
  edited mate; Esc; Enter accepts; amber moving part; limits per type (ui-parity asm-*-limits); a typed limit
  previews into range; the connector on a face's centroid / an edge's midpoint.
- List: the flyout shows the type alone until a parameter is set; double-click edits; row menus (instance, top,
  pattern, nested, two mates, mixed); the drop line switches at the row's middle, one undo; Shift+N renames an
  instance, not a mate.
- Insert: sort and remember the sort; Other documents; leaves out an empty Part Studio; follows the pointer, a click
  drops it; Esc drops the pending one, a second Esc closes with no Redo; ✕ offers Restore; ✓ deselects the first
  insert; search (ui-parity asm-insert-search); ✓ with one pending commits it at identity (asm-insert-ok-pending);
  thumbnails follow the chosen configuration.
