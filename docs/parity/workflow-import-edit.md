# Workflow: import & edit (a CAD user's session, step by step against live Onshape)

Document "qa5 import" (mm). Inputs: holeblock.step = block 40×20×10 with a Ø10 through hole at (20,10) (made with
noshape-cli, script /root/qa5-scratch/mkblock.py; = live's O1/O4 base), and live's own exports of the G24(b) part
(block 40×20×10, vertical edges Rc 2, top loop R 4): samples/g25-b.step / .x_t / .sat (live volume 7582.912205 mm³).
Live reference: behaviours.md "Geometry ground truth" (O1, O4, G24/G25), "Feature dialog basics", Export dialog
notes (G25). Geometry from the saved document via noshape-cli (same kernel).
Status: ✅ matches live, ✗ differs (finding #), ? not verified.

**Last run: 2026-10-21 (session 12:05–13:35), qa5**, binary /tmp/qa5/noshape-1206.

## 1. Import

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1.1 | + › Import… holeblock.step, Import to this document | Part Studio named after the file | tab "holeblock" (Import 1 → Part 1) + file tab in folder "CAD Imports"; 7214.601837 mm³ | ✅ |
| 1.2 | g25-b.step (live's AP242 export) | 7582.912205 | 7582.713024 (−0.199) | ? I1 |
| 1.3 | g25-b.x_t | 7582.912205 | 7583.000869 (+0.089) | ? I1 |
| 1.4 | g25-b.sat | 7582.912205 | **7585.978524 (+3.066)** | ✗ I1 |
| 1.5 | Second import of a same-named file | ? | tab "g25-b 1" | ? |
| 1.6 | Tool search "move face", Enter | dialog opens | same (search panel like live) | ✅ |

## 2. Direct edits on the imported body

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 2.1 | Move face: click near the edge of the x=40 face | field "Faces to move" only accepts faces (picks the face) | **chip "Edge of Import 1"** (feature red); 3 times in this run, also inside the hole | ✗ I2 |
| 2.2 | Move face, x=40 face, Offset 20 mm, flipped (O1) | **3607.3009183012764**, 1 part | **3607.300918**, 8 faces | ✅ |
| 2.3 | Move face, hole face 5 mm default direction (O4) | **8000** | 8000.0 (CLI; the hole wall could not be picked in the UI, see I2) | ✅ |
| 2.4 | … opposite direction | **2429.203673**, 1 part | 2429.203532, 1 part | ✅ |
| 2.5 | Feature row menu (last feature) | Rename, Edit…, Add selection to folder…, Suppress, Add comment, Zoom to selection, Show dependencies…, Add appearance…, Delete | same + Dynamic suppression | ✅ |
| 2.6 | Delete face (hole), Heal | 8000, 6 faces | 8000.0, 6 faces | ✅ |
| 2.7 | Replace face: top face, replace with Plane 1 (Top + 15) | ? (request) | plane refused, notice "Select faces." | ? I3 |
| 2.8 | Fillet (Shift+F) top front edge R3 | 8000 − (1−π/4)·9·40 = 7922.743339 | 7922.743339 | ✅ |
| 2.9 | Feature-list row tooltip after clicking a row (Plane: Top) | hides | "Top" tooltip stays over the open dialog's first label | ✗ I4 |

## 3. Export

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 3.1 | Part › Export… | File name + View export rules, Format (last used), ☐ Y axis up, Version (Parasolid list), ☐ Use latest version, Options | Parasolid, 38.1, ☑ Use latest version, Options Download | ✅ (format stickiness ?) |
| 3.2 | Parasolid file | header `SCH_3801231_38101` for 38.1 | `APPL=noshape … SCH=SCH_1200000_12006` (v12) although 38.1 was chosen | ✗ I5 |
| 3.3 | STEP export → re-import | same volume | 7922.743339 both ways | ✅ |
| 3.4 | Agent API import | (CLI/MCP must cover every UI feature) | **no import operation** in `noshape-cli tools` | ✗ I6 |

## Findings (2026-10-21 run)

| # | Finding | Repro | Owner |
|---|---|---|---|
| I2 | **Face-only query fields accept edges** (Move face "Faces to move"); with edges dominating near borders and inside holes, the hole wall can't be picked | Move face, click near a face border / inside a hole | psb |
| I1 | Imported volumes differ by format for the same part: SAT +3.066 mm³ (+0.04 %), X_T +0.089, STEP −0.199 vs live 7582.912205 | import samples/g25-b.* | kern |
| I5 | Parasolid export writes schema v12 while the dialog says 38.1 | Part › Export… Parasolid | kern |
| I6 | CLI/MCP has no import operation | `noshape-cli tools` | api |
| I3 | Replace face refuses a plane as the replacement (verify live) | §2.7 | psb |
| I4 | Feature-list row tooltip lingers over the dialog opened by the click | Plane / Sketch on Top | ui |

## Update from live's answer (behaviours.md "qa5 import/surfaces", 10-21)

| # | Row | Live | Verdict |
|---|---|---|---|
| 2.1 | Move face click 2 px from a border | picks the face (field is face-filtered); dialog type [Offset \| Translate \| Rotate], End type, Distance 5 mm, ☑ Reapply fillet | I2 confirmed |
| 2.7 | Replace face with Plane 1 | "Faces to replace with" (needs focus) accepts **Plane 1** (list or viewport); ✓ OK 12000 mm³ | **I3 confirmed** |

## Re-run pass 3 (2026-10-21 15:05–15:15; UI /tmp/qa5/noshape-1448 = target-shared 14:48, CLI /tmp/qa5/noshape-cli-1436)

| # | Row | Ours | Status |
|---|---|---|---|
| 1.2–1.4 | Fresh `import_files` of g25-b | SAT 7585.978524, X_T 7583.000869, STEP 7582.713024 (live 7582.912205) | ✗ I1 |
| 2.1 | Move face, click 2 px inside the x=40 face border | "Face of Import 1" (face picked) | ✅ I2 fixed |
| 2.9 | Row tooltip after clicking Top for a sketch | hides | ✅ I4 fixed |
| 3.4 | Agent API import | `import_files` exists (one tab per file, "g25-b", "g25-b 1", …) | ✅ I6 fixed |
| – | Switch to the holeblock tab | "Parts (0)" until a click (TAB1) | ✗ TAB1 |
| – | `noshape-cli … | head` | CLI **panics** "failed printing to stdout: Broken pipe" | ✗ **API1** (minor) |
| – | `tab_create kind="Part Studio"` (the kind document_info prints) | "unknown tab kind" (needs "partstudio"; the message doesn't list the valid kinds) | ✗ **API2** (minor) |
