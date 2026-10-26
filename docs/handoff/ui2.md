# ui2 handoff (2026-10-26, 18:00 UTC stop)

Area: Drawing properties panel (crates/app/src/drawing_drawprops.rs) and what it drives in
crates/model/src/drawing (build.rs, sheet.rs, tables.rs, views.rs, mod.rs); sketch-mode,
assembly / drawing / start-window visuals. Parity table: docs/parity/p2-look-assembly-drawing.md (DL9–DL14).

## On main
- DL12 Drawing properties, every tab wired to the sheet: Views (all groups), Annotations, Tables,
  Inspection (Text Bold default), Formats, Construction geometry.
- d045bf91 (+ merge d79ee626): Part Studio sketches in views (Solid / Construction pens, live
  dashes 8.5/1.4/2/1.4, Sketch points shape/size/colour on free points only), Sketch driven
  dimensions color (stored), cosmetic thread previews + Dashes only for ANSI + ISO/JIS arc gaps,
  Revision callouts (border / fit / font / text), Hole tables Indicator colour + Tags text.
- f9d174ed: fix main, `shown_sketches_overlay_part_views` updated to live's free-point markers
  (verified passing on that tree).
- Lock drawing properties (DL14), sketch vertex dots (builder.rs) landed earlier.

## WIP branch `agent/ui2` @ a6b4baa7 (clippy green, tests NOT run)
- Tables › Title row text: Bold (default on, live) / Italic / colour, model `TableStyle.title_text`,
  rendered in tables.rs; UI row with toggles + swatch (row_text_mut slots 8 / 9).
- Thread preview tiles: 1 px pixel-snapped hairlines with coverage alpha (0.48 px strokes were
  dropped by gpui's path rasterizer), tiles 2 px left (pl 77).
- Construction geometry: Extended lines row 44 px (pb 3) so Centermarks/Centerlines match live;
  Virtual sharps label centred on its select.
- Next step: `cargo test -p noshape-model --lib -- drawing::tests::table drawing::tests::grouped_revision
  drawing::tests::formats_gaps drawing::tests::bom_hole drawing::tests::custom_tables`, then
  rebase on main and push; screenshot with /root/ui2-scratch/dptabs.sh + dpscroll.sh 6:60 3:40.

## Open items (priority order)
1. Land the WIP branch (above). Then re-screenshot Tables end vs live
   drawing/ui2-ah-tab-Tables-end.png (Indicator lines swatch x 319, Revision callouts selects
   144..241 / 248..344) and the thread tiles vs ct-views-live (frame at x 79, y = Thickness row + 66 for ANSI).
2. Cosmetic threads ANSI end-on circle: live is 4 arcs with ~28° gaps (not our Hidden dash run);
   measure from drawing/ui2-ct-end-ANSI-2x.png and match.
3. Sketch driven dimensions color: wire once drawing sketches get driven dimensions (stored only).
4. Flat pattern Counterbore/Countersink up/down and Form tear/up/down styles are stored, not drawn.
5. Revision callout shapes other than Triangle reuse the Callout border geometry; compare with a
   live capture when one is available (REQUESTS.md).

## Known failing tests in my area
- None known. Main red at stop time was `assemblies_match_live` (assembly, not mine).
