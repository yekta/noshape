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

## Open: no test found yet (most used first)

Each still needs a test, or a fix if one fails.

**Dimensions and solver (lead's list):**
- Unsolvable dimension colours (behaviours 26111): which items go red, and the row name.
- Least-change probes 4 (31144): a rectangle plus a separate line, with Perpendicular / Parallel between them.
- Dimension right-click menu (36957).
- Sketch mode (116, 80 items): the base spec. It needs a per-item pass against its sketch/NN shots.

**Inference:**
- A click resting on a line's midpoint (33588, 34103, 34117): the midpoint rule.
- Sketch snapping and hover showing the constraint state (39769).
- Inference constants from live's client code (42894).

**Tools:**
- Line value box: typing after the second click (33862).
- Use with preselected edges or a face projects at once (23644).
- Sketch text made of lines and single quadratic B-splines (37537).
- Coverage batch 12 tools (36999).
- Region closing tolerance over free-end gaps (47283).

**Hover / select:**
- sketch-equal shows no constraint badges after step 11 (33657).
- qa5 sketch references and view helpers (6752).

**Settled without a test:**
- Tool hints (23116): live shows none, and we show none.
- Section base snap (14038, 16614): a section view, not sketching. Dropped from the count.
