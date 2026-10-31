# Workflow: behaviour sweep of the Part Studio features no qa workflow covered

psb, 2026-10-22. Sweep, Loft, Thicken, Wrap, Rib, Draft, Move face, Replace face, Delete face, Offset surface, Boundary surface,
Fill, Enclose, Helix, Composite part, Mate connector, Mirror / patterns of faces and features, Hole.
Compared: defaults, field order and labels, visibility, error texts and the field each error marks, and the result with
default options.
Sources: Onshape's FeatureScript std (`/root/onshape-research/docs-raw/fs-std/<feature>.fs`, texts from
`errorstringenum.gen.fs`), live dialog dumps (`live/dom/dialog-*.txt`), behaviours.md, help pages. Field order and labels
are also covered by `custom_features.rs::EXPECTED_IDENTICAL_DIALOGS` (our dialogs vs the std FS definitions).
Live request "psb (10-22) feature sweep" in live/REQUESTS.md, items L1–L61 below.
Status: ✅ matches, ✗ differs (open), 🔧 fixed on agent/psb, ? live pending (Lnn).

## Draft
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, visibility (Neutral plane / Parting line), angle 3° (0–89.9), Opposite direction ☐, second ☑, Tangent / Parting line propagation ☑, Reapply fillet ☐ | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty / unresolvable neutral plane | "Select a neutral plane." on Neutral plane | same text when empty; "The neutral plane could not be resolved" (no field) when it didn't resolve | ✅ (was 🔧) | skasm 10-31 56277472 (CLI: text and the field it marks, faulty_params) |
| 3 | Faces gone | "Select faces to draft." on Entities to draft | "The faces to draft no longer exist" | ✅ (was 🔧) | skasm 10-31 56277472 (CLI: text and the field it marks, faulty_params) |
| 4 | Parting line, no pull direction | "Select pull direction." on Pull direction | "Select the pull direction", no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI: text and the field it marks, faulty_params) |
| 5 | Parting line, no edges | "Select parting edges." on Parting edges | "Select the parting edges", no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI: text and the field it marks, faulty_params) |
| 6 | Kernel failure | "Selected faces could not be drafted." | "Draft failed: <kernel>" | ✅ (was 🔧) | skasm 10-31 (code: detail_a.rs DRAFT_FAILED is the kernel failure's message) |
| 7 | Parting edge parallel to pull | "Parting edge cannot be parallel to pull direction." on Parting edges | no check | ✅ (was 🔧 checked per edge before the planarity check, on Parting edges) | skasm 10-31 56277472 (CLI: text and the field it marks, faulty_params) |
| 8 | Parting line scope | any parting line (non-planar, several parts), each edge's faces drafted about the edge | planar lines perpendicular to the pull, one part ("Parting lines must lie in a plane…", "…must belong to one part") | ✅ (was 🔧 several parts (each on its own) and stepped / sloped straight parting edges (each edge's own hinge plane) (test `parting_line_draft_on_two_parts_and_steps`); curved non-planar parting lines still refused) | skasm 10-31 56277472 (T1/T2 parting_line_draft_sides_as_live, …_on_two_parts_and_steps, …_on_curves_as_live pass) |
| 9 | Two sided | first angle on the faces "away" from the pull, second on "along"; Opposite direction doesn't change which side One sided drafts | angle on the +pull side; sides follow the flipped pull | ✅ (was 🔧 live L1/L2: first angle below the parting line, second above, both narrowing toward it; each Opposite flips its side's taper only (test with live's 5 volumes)) | skasm 10-31 56277472 (T1/T2 parting_line_draft_sides_as_live, …_on_two_parts_and_steps, …_on_curves_as_live pass) |
| 10 | Edit logic | hint faces, focus on Entities to draft / Parting edges when editing | none; focus on Neutral plane | 🔧 INITIAL_FOCUS_ON_EDIT (Entities to draft / Parting edges focused when an existing draft opens); the first parting edge sets "Switch face" to the steeper face (flipAlongPull). Hint faces and the side-change flip adjustment ✗ (our planar parting lines decide the sides geometrically) |  |
| 11 | 0° draft, empty Entities to draft | — | — | 🔧 L3 0° = no change; L4 ✅ | 1b81b7ca 10-28 (blk40: neutral top face, front face, 0 deg → no error, 8000 mm³ unchanged (L3); the front face pick auto-advanced into Entities to draft) |

## Body draft
Live "psb (10-24 g) body draft" (block 40×20×10, pull +Z, parting Top, 5°).
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Parts to draft | the faces turn about their edge away from the parting entity: the top stays 40×20, the bottom grows by 10·tan 5° (8535.1377); Keep material makes no difference here | "not supported yet" | ✅ test `body_draft_as_live` | 67f88fd1 10-28 (blk40: Top plane parting 8535.138, top face parting the same) |
| 2 | Corner type Plane | corners bevelled from the top corner to the drafted faces' feet (8530.0348, 10 faces) | — | ✅ | 67f88fd1 10-28 (8530.035) |
| 3 | Symmetric | both sides narrow toward the parting plane (8265.0174, 10 faces) | — | ✅ (split at the parting plane, each side drafted, fused) | 67f88fd1 10-28 (Plane 1 at z 5, Symmetric 8265.017) |
| 4 | Faces to draft / Edges to hold | only the picked faces (8174.9773); held top edges = Parts | — | ✅ | 67f88fd1 10-28 (Faces to draft = front face, parting Top 8174.977; Edges to hold not run) |
| 5 | Cylinder | a cone r 10.8749 → 10 (3424.4619) | — | ✅ | b504234e 10-28 (Ø20 × 10 cylinder, Parts, pull and parting Top plane, 5° → 3424.462 mm³) |
| 6 | Errors | "Select parts to draft." / "Select faces to draft." / "Select edges to draft." on their field; "Body draft pull direction is invalid." on Pull direction; "Parting entity is invalid." on Parting entity; Draft on self of a block "Selected parts could not be drafted." on Parts | — | ✅ | 67f88fd1 10-28 (empty Parts / Faces / Edges: "Select parts to draft." / "Select faces to draft." / "Select edges to draft."; pull / parting / on-self texts not run) |
| 7 | Match faces at parting, concave repair, Draft on self that works, Keep material where drafting removes material | — | — | ✗ (no live case yet) |  |

## Move face
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, visibility, Distance 5 mm, Angle 0, Offset 0, Opposite offset ☑, Reapply fillet ☑ | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Check order | faces first: "Select faces to move." | direction / axis first | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 3 | No translation direction | "Cannot determine translation direction." on Direction | "Select a direction", no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 4 | Translate direction auto-fill | Direction = the first planar face picked (moveFaceEditingLogic) | stays empty | 🔧 a new Move face (Translate) takes the first planar moved face, until the user sets Direction |  |
| 5 | Rotate, no axis | text from evAxis | "Select a rotation axis" | ✅ (was 🔧 L5 "Cannot determine rotation axis." on Axis) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 6 | Up to entity, nothing / unresolvable | "Cannot resolve entities." on limitEntity | "Select the entity to move up to" | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 7 | Translate up to a plane | first face must be planar ("When moving up to a plane, first selected face must be planar.") and parallel ("Limit face must be parallel to face being moved.") | any plane, distance computed | ✅ (was 🔧 ✅ L6 (no field)) | skasm 10-31 56277472 (CLI direct.py): text, no field (L6) |
| 8 | Offset up to a non-parallel plane | the extended face meets the plane: distance 0 | distance along the normal | ✅ (was 🔧) | skasm 10-31 56277472 (CLI direct.py): Offset up to Right from the top face: OK, 8000 unchanged |
| 9 | 0 offset / translation | succeeds, part unchanged | went to the kernel | ✅ (was 🔧) | skasm 10-31 56277472 (CLI direct.py): Offset / Translate 0 mm OK, unchanged |
| 10 | Kernel failure | "Could not offset selected faces as requested." / "Could not transform selected faces as requested." | "Move face failed: …" | ✅ (was 🔧) | skasm 10-31 (code: direct.rs has both texts) |
| 11 | Reapply fillet | removes and re-rolls fillets touching the moved faces | ignored | ✅ (was 🔧 on by default: the fillets beside the moved faces are removed, the faces moved, the fillets rolled again at their radii (shared with Draft)) | skasm 10-31 56277472 (CLI direct.py): R3 edge fillet, front face offset 2 mm: 7922.7433 → 8722.7433 (+800), 7 faces (fillet re-rolled) |

## Replace face
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | Faces to replace, Faces to replace with, Flip alignment ☐, Offset distance 0 + flip | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty Faces to replace | "Select faces to replace." on Faces to replace | no period, no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 3 | Failure | "Could not replace faces as requested." | our own texts with kernel detail | ✅ (was 🔧) | skasm 10-31 (code: direct.rs) |
| 4 | Flip alignment | used; hidden when several replace-with faces; defaults on when the normals oppose (replaceFaceEditLogic) | ignored, always shown, always off | ✅ (was 🔧 L7/L8: wrong sense fails "Could not replace faces as requested.", offset along the template's (flipped) normal, hidden with 2 faces, auto-ticked when the normals oppose) | skasm 10-31 56277472 (T replace_face_flip_alignment_as_live, live L7/L8) |
| 5 | Several replace-with faces | used (must be adjacent, one body) | only the first | ✅ (was 🔧 the faces are sewn into one template (not joined: "Could not replace faces as requested.")) | skasm 10-31 56277472 (T replace_face_with_two_faces) |
| 6 | Empty replace-with | kernel text | "Select the face or plane to replace with" | ✅ (was 🔧 L9 "Cannot resolve entities." on Faces to replace with) | skasm 10-31 56277472 (CLI direct.py): "Cannot resolve entities." on templateFace |

## Delete face
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | Delete faces, Type Heal / Cap / Leave open (Heal), ☑ Delete fillet faces | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Failure | "Could not delete selected faces." (live) | "Delete face failed (…): <kernel>", cap texts | ✅ (was 🔧) | skasm 10-31 (code: direct.rs) |
| 3 | Every face picked | live: OK, the part disappears, no message | "Cannot delete every face of a part" | ✅ (was 🔧) | skasm 10-31 56277472 (CLI direct.py): all 6 faces → OK, no part, no message |
| 4 | Surface picked first | Type switches to Leave open (deleteFaceEditLogic) | stays Heal | 🔧 L10 |  |
| 5 | Delete fillet faces | fillets of any origin (imported parts) | only fillets our Fillet feature named | ✅ (was 🔧 L11: fillets recognised from the geometry (cylinder / torus / sphere / B-spline face tangent to a deleted face and to another), imported parts too; Heal without them fails "Could not delete selected faces." (test with live's 7922.7433)) | skasm 10-31 56277472 (T delete_face_removes_fillets_by_geometry_as_live, live L11: 7922.7433) |

## Sweep
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, defaults, Twist / Scale / Thin / Trim ends groups (dialog-sweep.txt) | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty path | "Select entities for sweep path." on path | same | ✅ | 67f88fd1 10-28 ("Select entities for sweep path.", Sweep path pink) |
| 3 | Thin, check order | wall checked before path: "Select faces or sketch regions to sweep." on wallShape | path first; own text, no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 4 | Path not one chain | "Sweep path curves are not all connected." on path | own text, no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks); no field, as live L13 |
| 5 | Lock direction empty | "Select direction to lock." on lockDirectionQuery | own text, no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 6 | Pitch revolutions limit | "Computed number of revolutions outside limit (between 0 and 1000). Current value is N." on pitch | no check | ✅ (was 🔧) | skasm 10-31 56277472 (CLI: Pitch 0.001 mm → the text with N, on pitch) |
| 7 | Failure | "Could not create valid swept body, check input." / thin "Failed to thicken swept body, check thickness." | "Sweep failed: …", own thin texts | ✅ (was 🔧 solid/surface; thin ✅ live "psb (10-24 c)" case 6: "Failed to trim result body, try deselecting "Trim ends" option." with Trim ends on an open path, else "Failed to thicken swept body, check thickness." (no field)) | skasm 10-31 (code: solid_a.rs texts; thin in thin_sweep_as_live) |
| 8 | Path opposite direction, Extend to full path | change where the sweep starts / whether it covers the path | ignored | ✅ (was 🔧 L12 (half toward the path's start / end; whole with Extend)) | skasm 10-31 56277472 (T sweep_from_the_middle_of_the_path_as_live, L12) |
| 9 | Thin method | sweep a sheet, thicken it, trim ends to planes | offset the wall, then sweep (scale scales the wall; Trim ends off ignored) | ✅ (was 🔧 live "psb (10-24 c)": walls square to the path without scale / twist keep the in-plane offset (identical); otherwise the sheet is swept, thickened normal to it (side 1 outside a closed wall / right of an open one along the path) and, with Trim ends on an open path, swept past the ends and cut to the cap planes; all eight live cases (outward / flip / mid plane / open line on −y / Scale 2 10264.8766 / oblique trimmed 7204.0248 and not 7203.9915 / arc 8685.2519 / region) in test `thin_sweep_as_live`) | skasm 10-31 56277472 (T thin_sweep_as_live, psb 10-24 c) |
| 10 | Empty profiles texts | — | own texts | ✅ (was 🔧 L13: solid/thin "Select faces or sketch regions to sweep." (field), surface "Select sketch curves to sweep." (no field), disconnected path no field, empty Faces to lock = None) | skasm 10-31 56277472 (CLI: solid on profiles, surface no field, L13) |

## Loft
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, defaults, End conditions, Path section count 5, Connections, isocurves 10 | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty profiles | "Select profiles in order from start to finish." on profiles | same | ✅ | 67f88fd1 10-28 ("Select profiles in order from start to finish.") |
| 3 | Guides, Path | supported | "…not supported yet" | ✅ (was 🔧 live "psb (10-24 f)". **Path**: the profiles carried along the path, Section count sections cut at even steps, live then runs a natural cubic through them (quarter arc □20: 25100.80 / 25128.64 for 5 / 10 sections); ours still comes out at the exact sweep (25132.6, within 0.13 %; the natural rebuild is rejected for path lofts, P3, test tolerance temporary); the path needn't touch the profiles (live: no error); two profiles only so far. **Guides** through polygon corners: each side face ruled between its corner columns (the guide or a straight line), sewn with the profiles; the guided edge is the guide; live's guided faces bulge a little more (one guide 5634.73 vs ours ~5628.8, four 9511.27 vs ~9511.44); "All guides should intersect each profile boundary." (no field); curved profiles, several guide curves, more than two profiles, guide continuity still "not supported yet") | skasm 10-31 56277472 (T loft_path_as_live, loft_guides_trims_and_continuity_as_live) |
| 4 | End conditions | applied, with their errors (planar profile, adjacent faces, tangency direction) | warning only | ✅ (was 🔧 Normal / Tangent to profile and Normal / Tangent to direction clamp the end derivative (kernel `LoftEnd`, natural_loft): live "psb (10-24) loft end derivative" (+ b): 3·ΔCP1 = m·(0.9 h + 0.1 c) at each corner column, a smoothstep between the corners, natural free end (Parasolid's interior columns carry a small extra bump: volumes within ~1.5e-5); two-profile lofts get a halfway section so OCCT smooths them; the nine live lofts (□→□ shifted / h 40 / Tangent, □→Ø16 m 0.5 / 3 / both ends / end only / Tangent 2) in test `loft_end_conditions_as_live`; texts "Only planar profiles are valid for Normal/Tangent to profile start/end condition.", "Select direction for start/end profile tangency."; Match tangent / curvature still a warning. **Tangent to profile is was 🔧**: corners match live, the interior columns don't (3900 vs live 3926.46, 4943.72 vs 4925.19); the test's 30 mm³ tolerance is temporary until live's columns ("psb (10-24 h)") land) | skasm 10-31 56277472 (T loft_end_conditions_as_live) |
| 5 | Connections | every connection; "Select a point on each profile to define a connection." | first only | ✅ (was 🔧 the text (on the array); two or more connections split the profiles at their points and loft connection to connection (kernel `connect_sections`); test `loft_between_squares`) | skasm 10-31 56277472 (T loft_between_squares) |
| 6 | Profile errors | "Point profiles can only exist as first or last profiles.", "Select a single sketch region or face per profile.", "Cannot use faces or regions with inner loops as profiles.", "Could not create valid profiles from selections." | own texts, no field | ✅ (was 🔧 ✅ L14 (no field)) | qa v0.4.14 10-29 (texts, no field as live L14) |
| 7 | Failure | "Could not create loft with given information. Check profile order, guide/profile intersections or end conditions." / thin "Failed to thicken lofted body, check thickness." | own texts | ✅ (was 🔧) | skasm 10-31 (code: solid_a.rs texts) |
| 8 | Trim profiles / guides, Thin Trim ends, Show isocurves | applied | Trim profiles / Trim guides applied (T loft_guides_trims_and_continuity_as_live, psb 10-26 c); a thin loft's Trim ends and Show isocurves not yet | ◐ | skasm 10-31 56277472 |
| 9 | One profile | allowed with a closed guide / path | error | ✅ (was 🔧 L15 "Select at least two profiles, a closed guide curve or a closed path." (no field); match connections still ? live) | skasm 10-31 (code: solid_a.rs, L15 text) |

## Thicken
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, 5 mm, Thickness 2 = 0, Keep tools ☐, merge scope, Mid plane swap | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty | "Select surfaces or faces to thicken." on entities | same | ✅ | 67f88fd1 10-28 ("Select surfaces or faces to thicken.") |
| 3 | Keep tools off, some faces of a surface | the surface is kept unless all its faces are picked | deleted when any face is picked | ✅ (was 🔧 L16) | skasm 10-31 56277472 (CLI: one face of a 2-face surface thickened, Keep tools off → the surface stays, L16) |
| 4 | Adjacent faces | one operation: one body | each face separately | ✗ L17: one L-shaped part of 2560 mm³ (square corner); ours 2 slabs — needs the kernel's intersection join (kern); overlapping slabs of one body are fused |  |
| 5 | Auto Add from a surface face | only faces of solids count (thicken.fs) | surface faces count (opens on Add) | 🔧 L18 (surface face stays New); a user-picked Add / Remove sets the flip (canSetBooleanFlip) |  |
| 6 | Errors | "Failed to thicken all selections." (likely) | "Thickness must be positive", "Thicken failed: …" | ✅ (was 🔧 L19 "Invalid input selections." (no field)) | skasm 10-31 56277472 (T thicken_as_live, L19) |

## Wrap
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | Creation type Solid / Surface / Split (Surface), operations for Solid, flip, Trim to target, anchors, Thickness 2.5 | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Position group | Angle / U shift / V shift in a collapsed "Position" group | always shown, no group | ✅ (fs: the std group, collapsed) | 67f88fd1 10-28 (Position group collapsed) |
| 3 | Filters | Tools planar only; Target cylindrical or conical only; no unwrap | wider, plus unwrap | 🔧 L20 (no unwrap; forced picks fail with live's texts) |  |
| 4 | Errors | "Select planar faces or sketch regions to wrap." (source), "Select a cylindrical or conical face to wrap around." (destination), "Source entities must all lie on the same plane.", "Select anchor points.", auto-anchor text, "Failed to imprint edges onto destination face.", ambiguous anchor | own texts, no fields | ✅ (was 🔧 (unwrap keeps its own texts)) | skasm 10-31 56277472 (CLI: source / destination texts on their fields; the other texts in surfacing_c.rs) |
| 5 | Dialog dump | — | — | 🔧 L21 Position group collapsed (FS compiler lacks the group, dialog comparison excludes wrap) |  |

## Rib
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, Thickness 2.5 mm, Parallel, Opposite direction ☑, Draft ☐ 3°, Extend ☐, Merge ribs ☑ | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty profiles | "Select sketch profiles to create the ribs." on profiles | same | ✅ | 67f88fd1 10-28 ("Select sketch profiles to create the ribs.") |
| 3 | Empty Parts | "Select parts where the rib will be fitted into." | all solids used | ✅ (was 🔧 L22 (no field); older ribs keep using every part) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks); no field, as live L22 |
| 4 | Parts / direction auto-fill | edit logic fills Parts and picks the side | none | 🔧 L22 Parts filled with the studio's parts on open |  |
| 5 | Closed profile, Parallel | "Only open sketch profiles can be used when rib direction is parallel to sketch plane." | no check | ✅ (was 🔧 ? L23) | core 82580226 10-29 (qa: the text); skasm 10-31 re-read |
| 6 | Draft, Extend profiles to part | supported | "not supported yet" / ignored | ✅ (Extend was 🔧 L22: parallel ribs are built as rib.fs does (swept past the part, split by it, excess removed; ends extended with the checkbox or when they sit on a face they point into), all five live rows match (3692, 3932, 4112, 4562, error). Draft was 🔧 rib.fs applyDraft: the sides drafted about the rib's top (wider into the part, narrower with Opposite), split by the part again; live G33c 27048.155585660817 matched (test `rib_draft_as_live`), Normal to sketch plane drafted the same way (no live number). Extend with Normal to sketch plane (L24: error) stays ✗) | skasm 10-31 56277472 (T rib_extend_profiles_as_live, rib_draft_as_live, rib_of_an_l_normal_to_its_plane_as_live) |
| 7 | Errors | "Failed to create a rib from a selected profile.", "None of the ribs intersected a part.", "Failed to merge ribs into parts." | own texts | ✅ (was 🔧) | skasm 10-31 (code: the texts) |
| 8 | One rib per edge | yes (Merge ribs off: one body per edge) | one per chain | ✅ L24: one body per profile chain (live: the L rib is one body); parallel needs one straight edge 🔧; Extend profiles see row 6 | v0.4.12 CLI 10-28 (bin/ribchain.py: two separate profiles, Merge off → Part 2 600 + Part 3 260, on → one part 10610; the L profile Normal fails → RIB-CHAIN-NORMAL, so live's L24 one-body case can't be checked) |

## Offset surface
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, 5 mm (0 allowed), Create selection, flip | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Nothing resolves | "Select faces, surfaces, or sketch regions to offset from." on surfacesAndFaces | "Nothing to offset"; one missing face fails the feature | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 3 | Kernel failure | — | raw kernel text | ✅ (was 🔧 L25 "Could not offset selected faces as requested."; L26 one surface per picked face) | skasm 10-31 (code: the texts) |

## Fill
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, Guides group, Sample size 5, isocurves 10, Merge with all ☑ | same | ✅ | 67f88fd1 10-28 (New / Add, Guides group (Vertices or curves, Precise), Show isocurves Count 10, Add: Merge with all ☑) |
| 2 | Empty edges | "Select laminar edges or curves as fill surface boundary." on edges | own text, no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI errsweep2.py: text and the field it marks) |
| 3 | Continuity label | "Continuity [Position]" label shown (SHOW_LABEL) | no label | 🔧 SHOW_LABEL |  |
| 4 | Loop errors | "A boundary edge can have only one type of continuity constraint.", "Cannot create fill surface between multiple loops.", "Boundary edges selected do not form a closed loop.", "Cannot create tangency or curvature constraint for a curve selection." | own texts | ✅ (was 🔧 ? L27 (internal-edge variant)) | skasm 10-31 (code: the texts) |
| 5 | Guides | live "psb (10-24 d)": Guides on with nothing picked is the plain fill; Precise passes through the guides (≤1.1e-4 mm), each touching the boundary at one end at least, else "Guide entities cannot be used as selected." (no field); arc + vertex: warning "Guide curves and vertices cannot be used in combination in Precise mode. Vertex selections are not used."; Sampled only approximates | not checked; always sampled | ✅ (was 🔧 all those rules (Precise curves as free MakeFilling constraints); test `fill_guides_as_fs`; our surface is OCCT's, so areas differ slightly (live arc 1845.4473)) | skasm 10-31 56277472 (T fill_guides_as_fs) |
| 6 | Add when the edges are surface edges | automatic (SF7) | no | ✗ (SF7) |  |

## Boundary surface
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, items, seven conditions (None), magnitude 1 | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Profile arrays | driven-query rows "[None] Edges of Curve U1" (live L28: no limit, a third row is taken) | "Add Profile" UI | ✅ driven query with the "[#uCondition] #uProfileEntities" item label | da18f886 10-28 (Boundary surface U profiles: rows "[None] Edge of Arcs sketch" ×2, a driven-query row per pick; see workflow-surfaces 11) |
| 3 | Boundary condition label | SHOW_LABEL | no label | 🔧 SHOW_LABEL |  |
| 4 | Too few profiles | "Select edges to specify at least two profiles." on both arrays | own text, no field | ✅ (was 🔧 L28; a third profile / mid-span crossing fail with live's texts was 🔧; 1U+1V was 🔧 builds the sheet the U sweeps along the V (the other two sides are the profiles moved to each other's far end; live 2099)) | skasm 10-31 56277472 (CLI err3.py: text and field) |
| 5 | Boundary conditions | applied, with their errors | warning only | ✅ (was 🔧 rows in one direction: the first / last row's condition is the loft end condition (same rule as Loft 4); live L28's "Only planar profiles are valid for Normal/Tangent to profile start condition." on a part edge (test `boundary_condition_needs_a_planar_profile`); U+V networks still warn) | skasm 10-31 (code: solid_a.rs's planar-profile text; loft end rule T loft_end_conditions_as_live) |
| 6 | Failure | "Could not create boundary surface with given information. Check profile order, intersections or end conditions." | own text | ✅ (was 🔧) | skasm 10-31 (code: the texts) |

## Enclose
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | Result body operation, Entities, Merge with all ☐, merge scope, Keep tools ☐ | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty | "Select faces, parts, or planes to form a solid." | own text | ✅ (was 🔧 L29 Entities marked) | skasm 10-31 56277472 (CLI err3.py: text and field) |
| 3 | No region | "Selections do not enclose a region." | own text | ✅ (was 🔧 ❌ (ENCLOSE-NO-REGION-MSG) L30) | skasm 10-31 56277472 (CLI err3.py: text and field); qa 58810bd9 |
| 4 | Failure | "Failed to create a part from enclosed region." | "Enclose failed: …" | ✅ (was 🔧) | skasm 10-31 (code: the texts) |
| 5 | Sketch regions | allowed | filtered out | ✅ (was 🔧 L31 accepted; enclosed regions make one part) | skasm 10-31 56277472 (CLI enclose.py: 7 planes → one part 2000; region + plane z10 → no region, as L31) |

## Helix
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, defaults (dialog-helix.txt) | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Axis empty | "Select an object with an axis." on axis | own text, no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI err3.py: text and field) |
| 3 | Circle empty | "Select a circle or arc." on edge | own text, no field | ✅ (was 🔧) | skasm 10-31 56277472 (CLI err3.py: text and field) |
| 4 | Height | 1e-5–500 m (0 out of range) | 0 allowed, flat spiral | ✅ (was 🔧 L32 "A parameter value is not valid." (any out-of-range value, all features)) | skasm 10-31 (code: eval.rs / feature_dialog.rs "A parameter value is not valid."; the CLI refuses the range first) |
| 5 | Zero radius | error only when both are 0 ("Start or end radius should be greater than 0."), else swapped | error when the start is 0 | ✅ (was 🔧 L33 start 0 with an end radius makes the conical spiral; both 0 fails) | skasm 10-31 56277472 (CLI err3.py: text and field): both 0 → the text; start 0, end 5 → OK |
| 6 | Start / end point | must be coplanar with the axis start / end (errors on startPoint / endPoint), snapped onto the radius | moves the plane, own errors | ✅ (was 🔧 helix.fs texts on startPoint / endPoint (end point only checked on a face: on an axis it sets the height); a point on a face or circle is moved onto the radius) | skasm 10-31 56277472 (T helix_points_as_fs) |
| 7 | Computed revolutions / pitch | filled | never | ✅ (was 🔧 filled when the end type is End point (helix.fs)) | skasm 10-31 56277472 (T helix_points_as_fs) |
| 8 | Show start and end profiles | magenta circles | nothing | ✅ (was 🔧 magenta debug circles (#ff2bc1) and centre dots (#e224e3) at both ends while the dialog is open (helix.fs drawConstructionEnds)) | skasm 10-31 56277472 (T helix_start_end_profiles: the circles and centres as debug geometry) |
| 9 | Preselection, start angle 0 | type from the preselection; canonical start angle | no; surface x direction | 🔧 L34 the preselection picks the tab (face → Surface, line edge / connector / sketch line → Axis, circle → Circle). L35 won't be matched: live's start angle follows Parasolid's own surface frames (a Z or X cylinder starts at the extrude's end, a tilted one at its base), which OCCT doesn't share |  |

## Composite part
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | Entities, Closed ☐; closed consumes members | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty | "Select entities to create a composite part." (likely) | own text, no field | ✅ (was 🔧 L36 Entities marked) | skasm 10-31 56277472 (CLI cmp.py: text on bodies, L36) |
| 3 | Overlapping composites | "Cannot create a closed composite part that partially overlaps another composite part." (and open) | no check | ✅ (was 🔧 L36 a part of a closed composite in another closed one fails with live's text; a whole composite nests) | skasm 10-31 (code: refgeom.rs's overlap text) |

## Mate connector (Part Studio)
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields, empty origin | order, labels; "Cannot resolve entities." on Origin entity | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Empty owner | "Cannot determine owner of mate connector." on Select owner entity | no error | ✅ (was 🔧 L37 (owner inferred from the origin's part or the only part, as the dialog does)) | skasm 10-31 56277472 (CLI cmp.py: origin with three parts → the text on ownerPart) |
| 3 | Owner / attachment auto-fill | from the origin entity or the only part (connectorEditLogic) | none | 🔧 L37 |  |
| 4 | To selection, empty | "Select an entity for the mate connector attachment." on Attach to | no check | ✅ (already in refgeom.rs; the doc was stale) | v0.4.12 CLI 10-28 (cent copy: Mate connector on the top face, attachmentOption TO_SELECTION, attachTo [] → Error "Select an entity for the mate connector attachment.", faulty attachTo) |
| 5 | Owner / attachment at regen | the connector follows them | not stored | ✅ (was 🔧 the owner part is kept with the connector and a Transform that moves the part moves it (attachment to a selection still follows the owner, not the selected face)) | skasm 10-31 56277472 (T mate_connector_follows_its_owner) |
| 6 | Move vs Reorient secondary axis | translation in the reoriented frame (likely) | translated first | ✅ (was 🔧 L38 order: flip/reorient, move, rotate; "+Y" = X to −Y) | skasm 10-31 56277472 (T mate_connector_frame_order_as_live, L38) |
| 7 | Between entities / Realign errors | built-in texts | own texts | ✅ (was 🔧 L39 "Failed to resolve mate connector coordinate system."; empty Realign axis = OK) | skasm 10-31 56277472 (CLI cmp.py: between, second empty → the text, no field; Realign with no primary → OK; L39) |

## Mirror / patterns: Face and Feature modes, Curve pattern
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | Face / Feature labels, no operation tabs, Reapply only for Feature (☐), defaults | same | ✅ | b504234e 10-28 (GUI: Feature mirror = Features to mirror, Mirror plane, ☐ Reapply features, no tabs; Face mirror = Faces to mirror (+tangent), Mirror plane, no tabs/Reapply; Part mirror New/Add/Remove/Intersect) |
| 2 | Empty Face mode | "Select faces to pattern." / "…mirror." on faces, before the direction | same | ✅ | 67f88fd1 10-28 (Face mirror "Select faces to mirror.", Face pattern "Select faces to pattern.") |
| 3 | Empty Feature mode | "Select features to pattern." / "…mirror." on Features, before the direction / plane | direction first, then "Select features to pattern" | ✅ (was 🔧) | skasm 10-31 56277472 (CLI pat.py: texts and fields) |
| 4 | Unresolvable plane / direction / axis | "Select a mirror plane." / "Select a pattern direction." / "Select a pattern axis." on the field | "…could not be resolved" | ✅ (was 🔧) | skasm 10-31 56277472 (CLI pat.py: texts and fields) |
| 5 | Linear second direction empty | error only when its count > 1: "Select a pattern direction." on directionTwo | always, own text | ✅ (was 🔧) | skasm 10-31 56277472 (CLI pat.py: texts and fields) |
| 6 | Reapply failures | "Could not pattern selected features." when all fail; partial failures silent | first failure's text; partial warning; geometric fallback | ✅ (was 🔧 texts and silent partial failures; no fallback to the geometry: a failing Reapply reports as is) | skasm 10-31 (code: pattern.rs "Could not pattern selected features." on instanceFunction) |
| 7 | Sketch in a feature pattern | re-run, info "Sketch dimensions and constraints using the Origin or the default planes as reference will not be reapplied. Learn more" | skipped | ✅ (was 🔧 L41 the sketch is copied to every instance (with or without Reapply; no part), Reapply adds the info) | skasm 10-31 56277472 (T feature_pattern_of_a_sketch_copies_it, L41) |
| 8 | Face pattern failures | "Failed to create pattern, check input.", "Pattern could not be created on the same part.", mirror "Failed to create face mirror, check input." | own texts; off-part instances become parts | ✅ (was 🔧 texts; L42 any face-pattern instance off the part fails "Pattern could not be created on the same part." (no part made); L40 a Feature pattern (Reapply off) skips one off-part instance and fails when all are off (text pending from live, ours the same)) | skasm 10-31 56277472 (CLI pat.py: texts and fields) |
| 9 | Feature selection in Faces to pattern | a feature-list click adds its faces | not supported | ✅ L44 (live adds nothing either; Hole not verified) | 8627322e 10-28 (wf-block: Linear pattern › Face pattern, Faces to pattern active, click the Extrude 1 row → the field stays empty = live) |
| 10 | Curve pattern | selection checked first; "Select a path to pattern along." on edges; "Edges do not form a continuous path."; distance-too-large text on distance; closed-path rule; start-point info | own texts, no fields | ✅ (was 🔧 incl. the start point: an open path starts at its end nearer the seed, a closed one at its point nearest the seed (path.fs), and a start outside the seed's box gives "For best results ensure that the path start point and pattern seed are touching." ("…the path and pattern seed…" when closed)) | skasm 10-31 56277472 (CLI pat.py: texts and fields) |
| 11 | Limits and infos | "Cannot have more than 2500 instances in a pattern.", parallel directions info, skip-instance infos | missing | ✅ (was 🔧 2500 limit (linear), parallel info, skip infos "Original pattern instance will not be skipped." / "Index is outside patterned instance count." (linear, circular, curve)) | skasm 10-31 56277472 (CLI pat.py: texts and fields) |
| 12 | Feature pattern of a wall flush with the part (Reapply off) | live "psb (10-23) flush wall pattern": a wall exactly on the end face fails "Could not create all instances…" from 3 instances; with 2 the end face moves by the distance (19700 mm³); an inward wall that adds nothing fails "Selected features do not create any geometry…"; any overhang fails at 2; "psb (10-24) flush wall follow-up": W4/W5 ×2 fail, a curve pattern along the same edge is the linear one (W1 ×2 OK 19700), a circular one (2° about Z) fails even at ×2 | flush walls patterned without error | ✅ a feature whose faces on the part are only caps across a pure translation (linear, or curve along a straight edge): ≥3 instances fail (ghosts shown), 2 move the caps (face offset); circular fails by the merged-face rule (a seed face reaching a part face's outline merges into it and is copied along: each copy must land in the plane of another part face facing the same way, as ADV9's mirrored gusset; onto its own plane (G45, W1–W5) or into empty space (the fins' front wall) it fails; a face wholly inside a part face, the narrow rib's bottom, doesn't count); tests `feature_pattern_of_a_flush_wall_as_live`, `feature_mirror_of_a_gusset_as_live` and kern's `pattern_fins` cover every live row | skasm 10-31 56277472 (T feature_pattern_of_a_flush_wall_as_live) |

## Hole
| # | Area | Onshape | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Fields | order, labels, enum texts, defaults, bounds, visibility (dialog-hole.txt) | same | ✅ | f9185639 10-28 (dialog fields) |
| 2 | Standard tables | holetables.gen.fs (already in crates/featurescript/std) | our own close values; missing sizes, pipe taps, PEM; labels like "1/64 (0.0156)" | ✅ (was 🔧 L58 read from holetables.gen.fs: every size, pipe tap and PEM entry, in live's order and with its keys ("1.00 mm (Coarse)", "1/4 (0.25)" before "E (0.25)"); paths saved with a bare pitch still resolve) | skasm 10-31 56277472 (T hole_dialog_table_values_then_edits, new_hole_follows_document_units_and_table; L58) |
| 3 | Tap drill default | 5 mm / 0.25 in | 4.2 mm / 0.201 in | ✅ (was 🔧 L58 values from the FS table) | skasm 10-31 56277472 (T hole_dialog_table_values_then_edits, new_hole_follows_document_units_and_table; L58) |
| 4 | Check order | counterbore, countersink, tolerances, tap drill, end, start, then "No hole points selected." | points first | ✅ (was 🔧) | skasm 10-31 56277472 (T hole_misses_and_order_as_onshape) |
| 5 | Counterbore / countersink errors | "The counterbore diameter must be larger than the hole diameter." (marks both), "The counterbore depth must be smaller than the hole depth.", "The countersink diameter must be larger than the hole diameter."; no countersink-depth error | own texts, no fields; countersink depth error | ✅ (was 🔧 texts and fields; the countersink-depth error stays ? L47) | skasm 10-31 56277472 (T hole_misses_and_order_as_onshape: counterbore text; the others in hole.rs; countersink depth ? L47) |
| 6 | Up to entity / selected plane empty | "Select an entity or mate connector to terminate the hole." / "Select a start entity." on the field | own texts, late | ✅ (was 🔧) | skasm 10-31 (code: hole.rs texts) |
| 7 | Merge scope | auto-filled with the parts the hole reaches, direction flipped toward them; empty: "No target parts selected for hole." | empty = every solid | ✅ (was 🔧 L56 empty scope: "No target parts selected for hole." on the scope; L57 a new Hole's scope is the parts it reaches and follows the depth until set by hand; holes saved before (ScopeRule 4) keep cutting every part. holeScopeFlipHeuristics: more parts behind the locations than in front flips a new hole (until the flip is set by hand), and a flip that reaches nothing clears the scope (live L57)) | skasm 10-31 56277472 (T hole_merge_scope_as_live, hole_scope_follows_depth_as_live; L56/L57) |
| 8 | Misses | "None of the holes intersected a part." (scope); warning "Some selections did not produce holes."; blind hole that misses must fail | own texts; silent; blind miss succeeds | ✅ (was 🔧) | skasm 10-31 56277472 (T hole_misses_and_order_as_onshape; a missing location fails the feature, live L53; the bottom-face point case: HOLE-MISS-MSG, live asked) |
| 9 | Tapped hole cut | Diameter, tap drill only with a fastener fit | tap drill always | ✅ (was 🔧 L51 (+ fit info bubble)) | skasm 10-31 56277472 (T hole_tapped_cuts_its_diameter, L51) |
| 10 | Up to next / entity + Offset from tip | the tip ends at entity − offset | the shaft ends there (one tip too deep) | ✅ (was 🔧 L61 apex at the offset, Depth shows 4.092) | skasm 10-31 56277472 (T hole_countersink_and_offset_as_live, L61) |
| 11 | Depth / Tapped depth / Tap clearance | kept consistent by edit logic; inconsistency warning | independent | ✅ (was 🔧 L49/L50 linked in the dialog (no mismatch warning, as live)) | skasm 10-31 56277472 (T tapped_hole_depths_stay_linked_as_live, L49/L50) |
| 12 | Infos | overridden parameters, fastener fit < 2 parts, tapped depth readjusted, too many locations (500, on locations) | missing / own text | ✅ (was 🔧 500-location text; L52 "Parameters have been overridden. Check input." when Diameter or Tap drill differ from the standard (not for counterbore / countersink); "Tapped depth readjusted. …" (tapped depth past the hole's depth in the part) and "Counterbore and countersink values are non standard for tapered holes."; the inconsistent-depth warning stays off as live L49/L50) | skasm 10-31 56277472 (T hole_overridden_parameters_info, hole_tapped_depth_readjusted_info, L52) |

## Re-run pass 21 (qa5, 2026-10-23, main 40e23871; source and app checks)

Fixed on main since the sweep: Wrap's collapsed "Position" group (row Wrap 2), Helix zero start radius swaps the ends (L33),
"Cannot create a closed composite part that partially overlaps another composite part." (L36), Mate connector "Select an entity
for the mate connector attachment." on Attach to (MC 4), pattern texts and the 2500-instance limit (🔧 rows).

Fixed by qa5 on agent/qa5 (each with a test):

| Row | Fix |
|---|---|
| Draft 7 | "Parting edge cannot be parallel to pull direction." on Parting edges (draft.fs getOrderedFaceData), checked before the plane test |
| Draft 10 (focus) | UIHint INITIAL_FOCUS_ON_EDIT: editing a Draft activates Entities to draft / Parting edges (also read from custom FeatureScript) |
| Move face 4 | Translate's Direction is filled with the first planar face to move until the user picks one (moveFaceEditingLogic) |
| Thicken 5 (flip) | switching the operation between New/Add and Remove/Intersect flips the wall unless flip / Thickness 2 / merge scope were set (canSetBooleanFlip) |
| Fill 3 | "Continuity" label shown (SHOW_LABEL) |
| Boundary surface 3 | "Boundary condition" label shown (SHOW_LABEL) |
| Helix 7 | Computed revolutions / Computed pitch filled with an End point (setFeatureComputedParameter) |

Still open, routed in docs/handoff/qa5-routing.md (psb): Draft 8 (parting line scope) and the hint faces, Move face 11
(Reapply fillet), Replace face 5 (several faces), Sweep 7/9 (thin method and texts), Loft 3/4/5/8 (guides, path, end
conditions, connections, trims), Thicken 4 (L17, kern), Fill 5/6 (guides, SF7), Boundary surface 2/5 (driven-query profile
arrays, boundary conditions), Helix 6/8 (start/end point rules, start/end profiles), Mate connector 5 (owner/attachment at
regen), pattern L40/L42/L43/L45 and skip-instance infos, Hole L56/L57 and the remaining infos.

## qa5 pass 22 (2026-10-24): labelled selects
std_show_label_hints_match_builtin (custom_features.rs) found 10 dialogs with selects the std labels (SHOW_LABEL) and we didn't:
bridgingCurve Method, constrainedSurface Optimize, endcap Treatment type, frame Default corner type / Corner type, gusset Gusset type /
Gusset position, isoparametricCurve Direction, offsetCurveOnFace Offset type / Gap fill, queryVariable Shared / Select ×2, routingCurve
Reference / Coordinate system, ruledSurface type / vertex override / corner type. All 🔧 fixed (qa5); the test keeps them aligned.

## Release re-check v0.4.8 (qa5, 2026-10-25 02:45, release asset noshape-cli 0.4.8 and target-engine 00:45; v048/tools/fsw.py)
| Row | Ours | Status | Verified |
|---|---|---|---|
| L56 Hole, Merge scope cleared (point on the block's top) | Error "No target parts selected for hole.", Merge scope flagged | ✅ | 98667558 10-28 (wf-block hole, Merge scope cleared: title red, with Sketch points active the empty Merge scope is #f9ebec with a red border, ✓ green = live 1533) |
| L45 Curve pattern, closed 100 mm path, Distance 25 × 4 | Error "Cannot create a curve pattern when the entered distance is larger than the path length.", Distance flagged | ✅ (NUMBER-FIELD-ERROR-MARK fixed a801dcd6) | d2357ee5 10-28 (Distance field #f9ebec + red rule when the pattern fails, = live 1533) |
| L45 Distance 25 × 3 | **Error** (same text); live OK with copies at 120° / 240° | ✅ **fixed** (was ✗ L45b, limit check on the sampled path length) | b504234e 10-28 (GUI: cpat, Part pattern › Distance 25 mm × 3 → OK, Parts (3) at 120° / 240°; fixed since 10-27 20:30) |
| L45b Distance 10 × 3 | copies at 120° / 240°; curvePattern.fs: 40° / 80° | ✅ **fixed** (was ✗ L45b; std spacing d·L / (L − d) on closed paths) | b504234e 10-28 (GUI edit to Distance 10 × 3 → copies at 40.0° / 80.0°, centres (12.186, 10.225) / (2.762, 15.666) = curvePattern.fs) |

## Re-check on the main CLI 12:08 (qa, 2026-10-27 20:30, v048/tools/fsw.py)
| Row | Ours | Status | Verified |
|---|---|---|---|
| L56 Hole, Merge scope cleared | Error "No target parts selected for hole." | ✅ | v0.4.12 CLI 10-28 (bin/l56.py: scope [] → Error 'No target parts selected for hole.', faulty scope; default scope Ok) |
| L45 Curve pattern d 25 × 4 on a 100 mm circle | Error "Cannot create a curve pattern when the entered distance is larger than the path length." | ✅ | v0.4.12 CLI 10-28 (bin/l45.py: Error with that text, faulty distance) |
| L45b × 3 | Ok, copies at 120° / 240° (centres (−7.958, ±13.783) on R 15.915) = live | ✅ **fixed** | v0.4.12 CLI 10-28 (bin/l45.py: Ok, centres (15.915,0), (−7.958, ±13.783)) |


## qa re-check 2026-10-29 09:36 (published v0.4.14 CLI, bin/errsweep.py: block 40×20×10, messages of the 🔧 rows; the field each error marks isn't in the CLI reply, not checked here)
| Row | Ours (v0.4.14) | Status | Verified |
|---|---|---|---|
| Draft 2 empty neutral plane | Error "Select a neutral plane." | ✅ | v0.4.14 10-29 |
| Draft 3 no faces | Error "Select faces to draft." | ✅ | v0.4.14 10-29 |
| Draft 4 parting line, nothing picked | Error "Select pull direction." (pull checked first) | ✅ | v0.4.14 10-29 |
| Move face 2 nothing | Error "Select faces to move." | ✅ | v0.4.14 10-29 |
| Move face 3 Translate, no direction (CLI, no editing logic) | Error "Cannot determine translation direction." | ✅ | v0.4.14 10-29 |
| Move face 5 Rotate, no axis | Error "Cannot determine rotation axis." | ✅ | v0.4.14 10-29 |
| Replace face 2 nothing | Error "Select faces to replace." | ✅ | v0.4.14 10-29 |
| Sweep 3 Thin, nothing | Error "Select faces or sketch regions to sweep." (wall before path; Solid / Surface check the path first: "Select entities for sweep path.") | ✅ | v0.4.14 10-29 |
| Sweep 5 Lock profile direction, no direction | Error "Select direction to lock." (also in the dialog on 349290dc: "Direction to lock" pink, path not) | ✅ | v0.4.14 10-29 |
| Rib 3 nothing | Error "Select sketch profiles to create the ribs." (profiles before parts) | ✅ | v0.4.14 10-29 |
| Offset surface 2 nothing | Error "Select faces, surfaces, or sketch regions to offset from." | ✅ | v0.4.14 10-29 |
| Fill 2 nothing | Error "Select laminar edges or curves as fill surface boundary." | ✅ | v0.4.14 10-29 |
| Wrap 4 nothing | Error "Select planar faces or sketch regions to wrap." | ✅ | v0.4.14 10-29 |
| Draft 7 parting edge parallel to pull (vertical edge, pull = top face) | Error "Parting edge cannot be parallel to pull direction." | ✅ | v0.4.14 10-29 |
| Rib 5 closed circle, Parallel to sketch plane | "Only open sketch profiles can be used when rib direction is parallel to sketch plane." (core 82580226) | ✅ | 82580226 10-29 |
| Move face 6 Up to entity with nothing | Error "Cannot resolve entities." | ✅ | v0.4.14 10-29 |
| Sweep 4 path of two disconnected lines | Error "Sweep path curves are not all connected." | ✅ | v0.4.14 10-29 |
| Loft 6 point profile in the middle | Error "Point profiles can only exist as first or last profiles." | ✅ | v0.4.14 10-29 |
| Loft 6 two regions in one profile | Error "Select a single sketch region or face per profile." | ✅ | v0.4.14 10-29 |
