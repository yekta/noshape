# Sketch hover × state matrix (skasm, lead P0 11-28)

Every sketch entity kind × state: our colour, width, glyph and pick priority against live. `=` live recorded and a test
of ours matches it; `?` live recorded, ours not yet checked by a test; `✗` differs; `L` live hasn't captured it (asked in
live/REQUESTS.md).

| kind | plain | hover | selected | sel+hover | multi-selected | under/fully defined | over-defined | face vs plane |
|---|---|---|---|---|---|---|---|---|
| line | ? | = | = | = | = | ? | L | L |
| arc | ? | = | = | ? | ? | ? | L | L |
| circle | ? | L | L | L | L | ? | L | L |
| spline | L | L | L | L | L | ? | L | L |
| point | ? | = | = | = | ? | ? | L | L |
| used curve | ? | L | L | L | L | ? | L | L |
| construction | ? | = | = | L | L | L | L | L |
| text | L | L | L | L | L | L | L | L |
| dimension | ? | = | = | L | L | L | L | L |
| constraint glyph | ? | ? | L | L | L | L | L | L |
| region | ? | ? | ? | ? | = | L | L | L |

Score (preliminary, 11-28 09:00 UTC): 14 =, 22 ?, 0 ✗, 52 L of 88.

## Live sources
- live/psui/hover-matrix-1121/index.md batch 2 (sk/*: point, line, line midpoint, arc, region × none / hovered / selected /
  selected+hovered / hovered with another selected; sketch on a plane): band 5 px #ffc685 (selected #ffa90a) with the
  blue line through its middle (#c79a96 / #c78436), no midpoint marker in sketch edit; point 11 px disc #fec788
  (selected #feab11, centre #ffd8ac / #ffc558); region: only its centre dot lights (#ffc686 / #ffa90a), fill unchanged.
- behaviours "sketch snapping and hover" (11-12): under-defined blue rgb(0,0,208), fully defined black, per entity.
- behaviours 168 / 17112: construction dash-dot in the same colour. Glyph hover (icons2 10-22): the glyph's background
  #fcda7f, its entities #ffc685 (secondary #f2ff80). Used geometry in the constrained colour (sketch/202).
- interaction-feel.md "Sketch" rows (psui): reach, priority, band middles, midpoint.

## Our tests
- GPU cells (part-studio-ui hover_matrix_tests): sk_point_{hovered,selected,selected_hovered},
  sk_line_{hovered,selected,selected_hovered,hovered_other_selected}, sk_linemid_*, sk_arc_{hovered,selected},
  sk_a_construction_lines_dashes_take_the_hover_and_selected_bands, sk_a_dimension_line_is_lives_gold_hovered_and_selected,
  sk_centres_and_regions_light_alike_whatever_else_is_selected.
- Colours: scene builder `sketch_curve_color` / `sketch_color` (Graphics preferences' under / constrained, SKETCH_OVER,
  construction alpha 200, used = constrained).

## Next
1. A scene test per row (colour per state from `sketch_color`, the pick states set by hover / selection, glyph sprites,
   pick priority), turning the `?` cells into `=` or `✗`.
2. Fix the `✗` cells.
3. REQUESTS for the `L` cells: spline, text, circle hover/selection, used curve hover/selection, over-defined geometry
   colour, multi-selection, a sketch on a face vs a plane, constraint glyph selected.
