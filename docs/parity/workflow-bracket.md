# Workflow: a full part from a drawing (bracket with holes, fillets, chamfer, pattern, mirror)

qa5, 2026-10-21 pass 3. Spec (same as live request "qa5 bracket" in REQUESTS.md): base 80×50×8; wall (0,0)–(80,8) × 60 Add;
2× Ø8 through holes at (20,30), (60,30); Fillet R5 on the vertical base edges at y=50; Chamfer 1 on the wall's top outer edge;
Ø6 wall hole at x=15, z=45 through the wall + Feature linear pattern 25 mm × 3 along X; gusset triangle on Right (8,8)–(30,8)–(8,40) × 5 Add;
Plane 1 = Right + 40; Feature mirror of the gusset. Hand-computed final volume **67191.3275 mm³**.
Built with the CLI (/root/qa5-scratch/pass3/mkbracket.py → /tmp/qa5/home/Documents/noshape/qa5 bracket.noshape, CLI 14:36),
then opened in the app (/tmp/qa5/noshape-1448). All times are **debug builds**; live times pending ("qa5 bracket").
Status: ✅ matches live/expected, ✗ differs, ? live data pending.

| # | Step | Expected | Ours | CLI call (s) | Status |
|---|---|---|---|---|---|
| 1 | Extrude base 8 | 32000 | 32000.0, 6 faces | 0.12 | ✅ |
| 2 | Wall 60 Add | 65280 | 65280.0, 8 | 0.21 | ✅ |
| 3 | 2 holes Remove Through all | 64475.7523 | 64475.7523, 10 | 0.22 | ✅ |
| 4 | Fillet R5 ×2 | 64389.9115 | 64389.9115, 12 | 0.32 | ✅ |
| 5 | Chamfer 1 | 64349.9115 | 64349.9115, 13 | 0.24 | ✅ |
| 6 | Wall hole + Linear pattern 25×3 | 63671.3275 | 63671.3275, 16 | 0.33 + 0.44 | ✅ |
| 7 | Gusset 5 Add | 65431.3275 | 65431.3275, 18 | 0.52 | ✅ |
| 8 | Feature mirror of the gusset about Plane 1 (Reapply features off, the default) | 67191.3275 (live result ?) | **Error "Could not create all instances as entered. Try selecting "Reapply features" option."**; with Reapply ☑: **67191.3275, 20 faces** | 0.41 | ✗ **ADV9** (? until live answers) |
| 9 | App: Features header stopwatch | live: "Show regeneration times" (per-feature times; Performance panel total) | floating "Regeneration times" popup: total **438.4 ms** on open (Extrude 2 74 ms, Extrude 5 75 ms, Mirror 92 ms) | – | ? (look + live times pending) |
| 10 | Edit Extrude 1 depth 8 → 10 (later features roll back while the dialog is open) | 73262.5555 by hand | **73262.555**, total regen **278.9 ms**, no errors | – | ✅ / ? time |
| 11 | Ctrl+Z | back to 8 | back to 8 | – | ✅ |

Findings: **ADV9** feature mirror of an Add extrude that touches two faces fails without Reapply features (psb; confirm with live step 8).

## Update from live's answer (behaviours.md "qa5 bracket", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| 1–8 | Volumes / faces | 32000 / 65280 / 64475.7523 / 64389.9115 / 64349.9115 / 63671.3275 / 65431.3275 / **67191.3275328646, 20 faces** | identical at every step | ✅ |
| 8 | Feature mirror, Reapply off | **OK** (Mirror 1 8 ms) | Error "Could not create all instances…" | ✗ **ADV9 confirmed** |
| 9 | Extrude 1 8 → 10 | 73262.5554193746 | 73262.555 | ✅ |
| – | Total regen after step 8 | **77 ms** (slowest Mirror 1 8 ms) | 438.4 ms on open (debug build; Mirror 1 92 ms, Extrude 2/5 ≈ 75 ms) | ✗ **PERF2** (re-measure on a release build) |
| – | After the 8 → 10 edit | 70 ms | 278.9 ms (debug) | ✗ PERF2 |
| – | "Regeneration times" dialog | 350 px floating dialog next to the feature list: table **# \| Feature name \| Time**, a **blue bar under each time**, **sorted by Time ▾ (slowest first)**, footer **"Total regen time: N ms"** | popup with rows in feature order, no #, no bars, no sort, total as a header line "Total regeneration time: 438.4 ms" | ✗ **REG1** |

## Agent CLI vs UI build (pass 4, 2026-10-21; UI /tmp/qa5/noshape-1448 in qa5-bracket-ui.noshape, CLI = mkbracket.py)

| Step | CLI | UI (by hand: sketch, g/c, d-dimensions, Shift+E) | Same? |
|---|---|---|---|
| 1 Base 80×50×8 | 32000.0, 6 faces | 32000.0, 6 faces | ✅ |
| 2 Wall 80×8 × 60 Add | 65280.0, 8 | 65280.0, 8 | ✅ |
| 3 2× Ø8 Remove Through all | 64475.7523, 10 | 64475.752281, 10 | ✅ |
| Tree names | Sketch 1, Extrude 1, Sketch 2, Extrude 2, Sketch 3, Extrude 3 | same | ✅ |
| Sketch state | CLI sketches **under-defined** (blue badge; no dimensions/constraints unless sketch_dimension/sketch_constrain are called) | fully defined (dimensions) | – (agent should add dims to match a hand build) |
| 4–8 | done in CLI (see above) | not repeated in the UI (picking the y=50 vertical edges took too many view changes; see notes) | – |

UI findings while building:
- **SK2** (skb): **F in a sketch fits the default plane's outline, not the sketch**: an 80 mm rectangle on Top ran off-screen after F; Onshape fits the sketch (and planes grow with the geometry). Also the view zoomed by itself after entering the first dimension.
- **RD1** (psb/ui, serious): the bottom-right **Volume readout is cached per part**: after editing Extrude 1 depth 25 → 8 it still showed 100000.000 mm³ (actual 32000, CLI) even after reselecting the part; same stale readout after the FeatureScript part disappeared (FS3).
- **SK3** (skb, ?): dimensioning from a sketch circle to a **model edge** creates "use"/projected curves (⧉ icons on 6+ edges, some extended past the part); Onshape references the edge without adding sketch geometry (live request "qa5 sketch refs").
- Extrude on the base's top-face area from Top view picks the face in front ("Face of Extrude 1"), not the Sketch 2 region below it: plausible (front-most wins); picking via the Sketch 2 row works.
- Remove extrude: Merge scope left empty, still removed from Part 1 ✅; Through all from the top face went into the material ✅ (live REST needed oppositeDirection; UI default unknown).
- View-cube ◁ ▷ arrows rotate ≈15° per click (live ?); Back view + clicking edge-on silhouette edges picks the end face first.
- Agent API can fully define sketches like the hand build: `sketch_draw corner_rectangle` from (0,0) already ties the corner to the origin and adds H/V; two `sketch_dimension kind=length value=…` calls → `dof 0`, the feature loses `underdefined` ✅ (pass3/clidims.py). Minor **API3**: `sketch_constrain coincident [corner, origin]` on that already-coincident corner fails with "a constraint needs two different entities" (misleading: it's already coincident). `sketch_dimension` without `value` → "required: Value or expression." (Onshape UI would take the current length; fine for an API but undocumented in the schema description).

## Update from live's answer (behaviours.md "qa5 sketch refs", 10-21)
- SK3 **confirmed**: Onshape creates **no sketch geometry/icons** for a dimension to a model edge (only a point-to-edge dimension; an implicit midpoint snap stays as a small white dot).
- SK2 **confirmed**: F fits model + planes + sketch; the Top outline doesn't grow, but while editing Onshape draws a larger pale-blue "Sketch N" plane outline around the geometry (ours: no such outline, and F ignores the sketch).
- View cube arrows: 15° per click (ours ≈15° ✅).
- Extrude after closing a one-region sketch: region **not** preselected, ✓ disabled (ours same ✅).
- Remove + Through all: Merge scope **auto-filled "Part 1"** (ours empty ✗ **EXT1**, minor), default direction into the material (ours ✅).
