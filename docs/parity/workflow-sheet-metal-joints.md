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

## Update from live's answer "qa5 sm tools" (behaviours.md ~7995) + CLI pass 20 (/root/qa5-scratch/pass20/smtools.py, CLI 08:11)
| Row | Live | Ours | Status |
|---|---|---|---|
| 7 Tab dialog | Tab profile (required, pink when unfocused) · Flange to merge · Subtraction offset 0 mm · Subtraction scope | same fields (jd-tab) | ✅ (pink state not checked) |
| 8 Rip | **there is no Rip tool** (tool search: "No items match"); rips only as Joint type Rip | our tool search opens a **"Rip 1"** feature (Vertex pairs to split with, ☑ Use default minimal gap) | ✗ **RIP1** (fs/ui: an extra tool) |
| 9 Bend dialog | Bend line + flip arrow · Sheet metal face to bend · **Bend alignment ▾** Bend line \| Hold line \| Hold other line \| Inner \| Outer \| Middle · ▾ Bend angle \| Align to geometry \| Angle from direction + ↻ · 90 deg · ☑ ☑ | Bend line has an **extra clock icon**; first select has no "Bend alignment" label (jd-bend); option lists not checked | ✗ minor (BEND2) |
| 9b Bend result (plate 60×40 t2 r2, line x=30 on the top face) | 4812.566370614357 mm³, 14 faces; **x<30 stays flat**, the x>30 part goes up (wall x 29.72..31.72) | **4812.566371, 14 faces** ✅ but bbox x 28.28..60: the **x<30 part goes up** and x>30 stays flat | ✗ **BEND1** (fs: wrong side held) |
| 10 Jog dialog | … Bounding type ▾ Blind \| Up to entity \| Thickness · **Jog offset 5 mm** · Jog offset anchor ▾ Inside \| Nominal \| Outside · ☑ Preserve material | Jog offset **10 mm** (jd-jog; CLI default bendOffset 10 mm) | ✗ **JOG1** (fs) |
| 10b Jog result (defaults) | 4825.132741228719 mm³, 22 faces; raised flange z 7..9, x 33.72..55.889 | **4825.132741, 22 faces** ✅; bbox z 0..14 (10 mm offset) and min x 9.11 (the x<30 side is raised, as in BEND1) | ✗ JOG1 + BEND1 |
| Tab result (rect (60,10)–(75,30) on the top face) | — | OK, 5400 mm³ (4800 + 600), 12 faces | ? (live volume not given) |

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
| Row | Ours | Status |
|---|---|---|
| SML1 loft Add | OK, one body **9805.524423, 14 faces** (= live 9805.524422649518) | ✅ **fixed** |
| BEND1 | 4812.566371, 14 faces, bbox x 0..31.72 (x<30 flat, wall x≈29.7..31.7) | ✅ **fixed** |
| JOG1 | 4825.132741, 22 faces, bbox max x 55.889, z 9 | ✅ **fixed** |
| RIP1 | (fixed in cc84d44: tool search lists only tools) | ✅ |

## Re-run pass 21 (qa5, 2026-10-23, main 40e23871 app /root/target-lead/debug/noshape)
| BEND2 | Bend line without the mate-connector button; "Bend alignment" label shown (SHOW_LABEL) | ✅ fixed (code; std sheetMetalBend.fs) |
