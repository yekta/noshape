# Workflow: sheet-metal joints, loft and less common sheet-metal tools (Make joint, Modify joint, Sheet metal loft, Tab, Rip, Bend, Jog)

qa5, 2026-10-22 pass 16, app /tmp/qa5/noshape-0503 (main aa75fdf), CLI /tmp/qa5/noshape-cli-0521.
Docs: /tmp/qa5/d27/loftADD.noshape, loftNEW.noshape (setup /root/qa5-scratch/pass16/mksmloft.py = live "kern G34 d"), /tmp/qa5/d27/sm.noshape (copy of "qa5 sheet metal").
Live: behaviours.md G34 (~905–918: Corner Closed, Modify joint butt styles, Make joint dialog, Sheet metal loft), "qa5 sm" (8) Sheet metal table and flat view (~5860).
Status: ✅ matches live, ✗ differs, ? live pending. Shots /tmp/qa5/shots/j01–j07, jd-*.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Sheet metal loft **New** (plate 60×40 t2 r2 + line (0,70)–(60,70) on Plane z30; profile 1 = plate top edge y=40) | separate body, 6 PLANE faces, loft wall **4924.38828688397** mm³ (total 9724.388) | Part 2 **4924.388287**, 6 faces (CLI) | ✅ |
| 2 | Sheet metal loft **Add**, merge scope = the plate | OK, one body 9805.524422649518 mm³, 14 faces; plate trimmed to y=39.2117 and a bend added | Error **"Adding a sheet metal loft to an existing model is not supported yet; use New"** | ✗ **SML1** (fs/kern) |
| 3 | Make joint dialog (tool search) | "Edges or side faces to join" · Joint type ▾ Bend \| Rip (default Rip) · Joint style ▾ Edge joint \| Butt joint - Direction 1 \| Butt joint - Direction 2 · slider · ? (part-studio/532–534) | identical (cmp-j01, j02, j03) | ✅ |
| 4 | Make joint after Finish sheet metal model, an edge picked | (live tried before Finish: "Cannot add joint joining selected entities.") | red "Make joint 1 did not regenerate properly: Part 1 is no longer an active sheet metal model (Finish sheet metal m…" — sensible for a finished model (j05) | ✅ (not the live case) |
| 5 | Modify joint butt styles after a Closed corner (live 8601.722 / 8601.614) | — | not testable: Closed corner unsupported (SM3) and the base flange differs (SM1) | blocked by SM1/SM3 |
| 6 | Sheet metal table and flat view panel | "Sheet metal context ▾", ⌄ Bends grid (#, Name, Radius, Angle, Bend direction, K Factor), ⌄ Other joints grid (Name, Type ▾, Style ▾), lower half shaded 3D flat viewport with its own view cube (541, "qa5 sm" §8) | still the old design: "Sheet metal" tab, white line drawing, summary rows, Dir/Angle/Radius/Allow table, Export DXF / Export SVG buttons (j06) | ✗ **SM4** still (fs + ui) |
| 7 | Tab dialog | ? (asked) | Tab profile · Flange to merge · Subtraction offset 0 mm · Subtraction scope (jd-tab) | ? |
| 8 | Rip dialog | ? (asked) | Vertex pairs to split with · ☑ Use default minimal gap (jd-rip) | ? |
| 9 | Bend dialog | ? (asked) | Bend line (+ clock, flip) · Sheet metal face to bend · ▾ Bend line · ▾ Bend angle (+ ↻) · Bend angle 90 deg · ☑ Use model bend radius · ☑ Use model K Factor (jd-bend) | ? |
| 10 | Jog dialog | ? (asked) | as Bend + ▾ Blind · Jog offset 10 mm · ▾ Inside · ☑ Preserve material (jd-jog) | ? |

Findings: SML1 (new). SM4, SM1, SM3 still. Live request "qa5 (10-22 pass 16, workflow 27 …)" filed for rows 7–10.

## Re-run pass 18 (2026-10-22 ~10:00–11:00, CLI /tmp/qa5/noshape-cli-0811 = target-engine 08:11; main 3ce6952)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS; SML1 loft Add "not supported yet". ✗ still.
