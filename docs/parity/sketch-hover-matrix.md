# Sketch hover × state matrix (skasm, lead P0 11-28)

Every sketch entity kind × state: our colour, width, glyph and pick priority against live. `=` live recorded and a test
of ours matches it; `?` live recorded, ours not yet checked by a test; `✗` differs; `L` live hasn't captured it (asked in
live/REQUESTS.md).

| kind | plain | hover | selected | sel+hover | multi-selected | under/fully defined | over-defined | face vs plane |
|---|---|---|---|---|---|---|---|---|
| line | = | = | = | = | = | = | = | L |
| arc | = | = | = | = | = | = | = | L |
| circle | = | = | = | L | = | = | = | L |
| spline | L | L | L | L | L | = | = | L |
| point | = | = | = | = | = | = | = | L |
| used curve | = | L | L | L | L | = | L | L |
| construction | = | = | = | L | L | = | L | L |
| text | L | L | L | L | L | L | L | L |
| dimension | = | = | = | = | L | L | L | L |
| constraint glyph | = | = | = | L | L | L | = | L |
| region | = | = | = | = | = | L | L | L |

Score (11-28 09:3x UTC): 48 =, 0 ?, 0 ✗, 40 L of 88 (preliminary 09:00: 14 =, 22 ?). "multi-selected" is hovered with
another entity selected (live's `-hovered-other-selected`); the circle's is the selected circle under another's hover.
The 40 L cells are asked in live/REQUESTS.md "skasm 11-28 09:2x" (live batch 12).

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
- 11-28 cells (live sk/*, measured by script): sk_{point,line,arc,region}_none, sk_point_hovered_other_selected,
  sk_arc_{selected_hovered,hovered_other_selected}, sk_circle_{hovered,selected} (hovered: live b5 Dimension tool),
  sk_region_{hovered,selected,selected_hovered,hovered_other_selected}; dimension: sk_a_dimension_line_is_lives_gold_hovered_and_selected
  (rest / hovered / selected / both).
- Colours: scene `sketch_lines_take_lives_colours_and_widths` (under #0000d0, full #000, over #cd0000 (live behaviours
  "conflicting rectangle draws red"), used = constrained, construction keeps the colour); every kind's curve and point
  goes through `sketch_color` (used points too, since 11-28).
- Glyph: `a_constraint_glyphs_box_takes_lives_colour_in_each_state` (live bundle tokens, live-exploration §1.9: white,
  #fcda7f, #fab601, conflict #d66e74 / #aa252d / #bd3039 with white icons, external blues).

## Next
1. Live batch 12 (the L cells), then a cell per new take.
2. Open from the bundle tokens: "Hovered glyph border rgb(255,169,10) 2 px" (ours has none: check it
   against batch 12's glyph shots).
