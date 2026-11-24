# Sketch feel audit (skasm, 2026-11-24)

The lead (11-24) asked for psui's kind of count for sketching: behaviours.md's sketch specs (tools, constraints,
dimensions, inference, drags, hover/select, solver) against our tests, covered before and after, tests added for the
uncovered ones (most-used first) and what fails fixed. Scripts: /root/skasm-scratch/sfa/ (scan.py, classify.py,
mapped.py, kw.py).

## Method

- **Specs:** behaviours.md `## ` sections whose title is about sketching (sketch, constraint, dimension, inference,
  snap, solver, glyph, a sketch tool's name). Sections about drawings, assemblies, sheet metal, Booleans, standard
  content, sketch imports and loft geometry are left out (46 by hand). That leaves **148 sections with 2518 items**
  (bullets and table rows).
- **Covered:** our tests (part-studio-ui `*tests*.rs`, the scene sketch_mode tests, crates/sketch tests, the ops
  tests) cite the section by its capture name (`sketch-tools-1123`, `live/skasm/<dir>/`), its ticket, its request
  number (`live 2022`, `request 1511`) or its title. A row of docs/parity/interaction-feel.md that names the section also
  counts: each of those rows names its test. Sections with none of these were then checked by hand, grepping the tests
  for what they describe.
- Counted per section: a section counts as covered when its main behaviour has a test, even if a side item has none.
  The open items are listed below.

## Count (sections / items)

| area | sections | items | covered before (sections) | items in covered sections |
|---|---|---|---|---|
| tools | 36 | 444 | 19 | 244 |
| dimension | 23 | 507 | 13 | 235 |
| solver (drags, least change, conflicts) | 20 | 388 | 16 | 324 |
| inference / snaps | 22 | 378 | 12 | 222 |
| hover / select / glyphs | 21 | 344 | 14 | 204 |
| constraint | 8 | 147 | 2 | 38 |
| other (visibility, menus, readouts) | 18 | 310 | 12 | 96 |
| **total** | **148** | **2518** | **88** | **1363** |

By citation alone, 88 sections are covered. Checking the other 60 by hand found tests for many that cite the spec in
other words, for example:
- qa5 dim conflict: an unsolvable value makes everything red, and Esc reverts the box (sketch_mode tests at the conflict
  label);
- live 2022: a free line's length edit moves its start (onshape_ux.rs);
- drag probes 7/9/12 (onshape_ux.rs drag_probes);
- live sketch-state 11-17: free → FIX → conflicting HORIZONTAL → deletes (an_open_sketchs_state_follows_its_constraints);
- value-box Tab (rectangle_value_boxes_tab_as_live);
- polygon side count (polygon_sides_typed_before_the_rim_click);
- offset constraints (offset_copies_follow_their_sources).

## Added (after)

- **qa sketch.md 7.14** (request 1511), solver: a third, redundant dimension is created driven. A horizontal 40 and a
  vertical 30 on a free line drive, the line's aligned 50 is driven and nothing moves. Test:
  `a_third_redundant_dimension_is_created_driven` (scene).
- **SKT-DASHDOT-GUIDES** (dashdot-guides-1124), tools: the centre point arc's dash-dot radius and circle, gone at the
  start click. Test: `a_center_arc_shows_its_dash_dot_circle_and_radius_until_the_start_click`.
- **Unsolvable dimension colours, live 1632 (26111)**, dimension edits: R6 → R20 on a 20 × 10 rectangle's Tangent
  corner arc solves by flipping, no error (`a_corner_arcs_radius_past_the_rectangle_solves_by_flipping`, scene). The
  red / bubble / row colours were already tested (`an_open_sketchs_state_follows_its_constraints`, ops
  `a_dimension_value_the_sketch_cant_take_is_kept_driving`). Live's Ø on a circle Tangent to two lines 10 apart errors
  at once, though the 10 it measures is what the rest fixes: a live failure, not a target (parity rule). Ours creates
  it driven, as live does a second Ø (`a_diameter_fixed_through_tangents_is_created_driven`).
- **Dimension right-click menu (36957)**: the rows in live's order, Add comment (cloud) left out
  (`a_dimensions_menu_as_lives`, GPUI).
- **Midpoint rule (33588, 34103, 34117)**: a rest on a sketch line's midpoint lights the line, shows no marker; the
  click selects the line (`a_click_resting_on_a_lines_midpoint_selects_the_line`, scene).
- **Line value box (33862)**: no box after one click; after the second, with the pointer moved, 50 + Enter dimensions
  the committed line, no new segment (`a_lines_value_box_waits_for_its_second_click`, scene).
- **Use with a preselection (23644)**: a selected model edge, then u, projects it at once, clears the selection, no
  tool left on (`use_with_a_preselected_edge_projects_it_at_once`, GPUI).
- **Sketch text (37537)**: outlines are only lines and degree-2, 3-point, plain B-splines (text_and_tools).
- **Region closing tolerance (47283)**: free ends close within 0.2 µm at any scale, not 0.3; a T-end at 0.01 µm, not
  0.02 (`free_ends_close_regions_within_0_2_um`, sketch regions). Fixed: ours joined ends only within 4 nm.
- **Least-change probes 4 (31144)**: all 8 cases match live; T1
  `a_rectangle_tied_to_a_separate_line_keeps_its_lower_left_corner` (live_fuzz).
- **Constraint state (39769)**: an under-defined sketch's row tooltip "Sketch 1 is not fully defined"
  (`an_under_defined_sketchs_row_says_so`, GPUI). Snap radii, midpoint snap and colours were already tested
  (snap_matrix_tests, sketch_ui_tests drag-onto-line, solver colours).
- **qa5 sketch refs (6752)**: Extrude right after closing a one-region sketch opens empty, ✓ disabled
  (`extrude_after_closing_a_sketch_starts_empty`, GPUI). The dimension to a model edge was tested (qa5 SK3).

- **Sketch mode, leaving (116)**: ✕ on a new sketch shows "Sketch 1 has been canceled."; Restore reopens it
  (`a_canceled_new_sketch_says_so_and_restore_reopens_it`, GPUI). **Tool states (sketch/200-233)**: L, L, L toggles
  Line; L with a line selected clears it; Enter with no pending segment does nothing; Esc exits
  (`a_tools_shortcut_again_turns_it_off_and_clears_the_selection`, GPUI).

**Found covered on a closer look:** inference constants (42894: the sticky target, sketch_mode
`…lastInferenceTarget` test); sketch-equal badges (33657: sketch_glyph_feel_tests); coverage batch 12 (36999: psui's
ui-parity coverage scenarios replay it).

## Open: no test found yet

- **Sketch mode (116, 80 items)**: the base spec (entering / leaving, colours, each tool's clicks). Most items have
  tests under other names (colours: hover_matrix, the preview blue; tools: sketchtools_feel). Per-item pass under way:
  done for cancel / Restore and the tool shortcut toggles (Added); left: spline Esc, Line mid-chain drag, Esc with a
  menu open, the camera at sketch start.

**Settled without a test:**
- Tool hints (23116): live shows none, and we show none.
- Section base snap (14038, 16614): a section view, not sketching. Dropped from the count.
