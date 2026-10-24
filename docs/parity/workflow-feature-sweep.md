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
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, visibility (Neutral plane / Parting line), angle 3° (0–89.9), Opposite direction ☐, second ☑, Tangent / Parting line propagation ☑, Reapply fillet ☐ | same | ✅ |
| 2 | Empty / unresolvable neutral plane | "Select a neutral plane." on Neutral plane | same text when empty; "The neutral plane could not be resolved" (no field) when it didn't resolve | 🔧 |
| 3 | Faces gone | "Select faces to draft." on Entities to draft | "The faces to draft no longer exist" | 🔧 |
| 4 | Parting line, no pull direction | "Select pull direction." on Pull direction | "Select the pull direction", no field | 🔧 |
| 5 | Parting line, no edges | "Select parting edges." on Parting edges | "Select the parting edges", no field | 🔧 |
| 6 | Kernel failure | "Selected faces could not be drafted." | "Draft failed: <kernel>" | 🔧 |
| 7 | Parting edge parallel to pull | "Parting edge cannot be parallel to pull direction." on Parting edges | no check | 🔧 checked per edge before the planarity check, on Parting edges |
| 8 | Parting line scope | any parting line (non-planar, several parts), each edge's faces drafted about the edge | planar lines perpendicular to the pull, one part ("Parting lines must lie in a plane…", "…must belong to one part") | ✗ |
| 9 | Two sided | first angle on the faces "away" from the pull, second on "along"; Opposite direction doesn't change which side One sided drafts | angle on the +pull side; sides follow the flipped pull | 🔧 live L1/L2: first angle below the parting line, second above, both narrowing toward it; each Opposite flips its side's taper only (test with live's 5 volumes) |
| 10 | Edit logic | hint faces, focus on Entities to draft / Parting edges when editing | none; focus on Neutral plane | 🔧 INITIAL_FOCUS_ON_EDIT (Entities to draft / Parting edges focused when an existing draft opens); the first parting edge sets "Switch face" to the steeper face (flipAlongPull). Hint faces and the side-change flip adjustment ✗ (our planar parting lines decide the sides geometrically) |
| 11 | 0° draft, empty Entities to draft | — | — | 🔧 L3 0° = no change; L4 ✅ |

## Move face
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, visibility, Distance 5 mm, Angle 0, Offset 0, Opposite offset ☑, Reapply fillet ☑ | same | ✅ |
| 2 | Check order | faces first: "Select faces to move." | direction / axis first | 🔧 |
| 3 | No translation direction | "Cannot determine translation direction." on Direction | "Select a direction", no field | 🔧 |
| 4 | Translate direction auto-fill | Direction = the first planar face picked (moveFaceEditingLogic) | stays empty | 🔧 a new Move face (Translate) takes the first planar moved face, until the user sets Direction |
| 5 | Rotate, no axis | text from evAxis | "Select a rotation axis" | 🔧 L5 "Cannot determine rotation axis." on Axis |
| 6 | Up to entity, nothing / unresolvable | "Cannot resolve entities." on limitEntity | "Select the entity to move up to" | 🔧 |
| 7 | Translate up to a plane | first face must be planar ("When moving up to a plane, first selected face must be planar.") and parallel ("Limit face must be parallel to face being moved.") | any plane, distance computed | 🔧 ✅ L6 (no field) |
| 8 | Offset up to a non-parallel plane | the extended face meets the plane: distance 0 | distance along the normal | 🔧 |
| 9 | 0 offset / translation | succeeds, part unchanged | went to the kernel | 🔧 |
| 10 | Kernel failure | "Could not offset selected faces as requested." / "Could not transform selected faces as requested." | "Move face failed: …" | 🔧 |
| 11 | Reapply fillet | removes and re-rolls fillets touching the moved faces | ignored | 🔧 on by default: the fillets beside the moved faces are removed, the faces moved, the fillets rolled again at their radii (shared with Draft) |

## Replace face
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | Faces to replace, Faces to replace with, Flip alignment ☐, Offset distance 0 + flip | same | ✅ |
| 2 | Empty Faces to replace | "Select faces to replace." on Faces to replace | no period, no field | 🔧 |
| 3 | Failure | "Could not replace faces as requested." | our own texts with kernel detail | 🔧 |
| 4 | Flip alignment | used; hidden when several replace-with faces; defaults on when the normals oppose (replaceFaceEditLogic) | ignored, always shown, always off | 🔧 L7/L8: wrong sense fails "Could not replace faces as requested.", offset along the template's (flipped) normal, hidden with 2 faces, auto-ticked when the normals oppose |
| 5 | Several replace-with faces | used (must be adjacent, one body) | only the first | 🔧 the faces are sewn into one template (not joined: "Could not replace faces as requested.") |
| 6 | Empty replace-with | kernel text | "Select the face or plane to replace with" | 🔧 L9 "Cannot resolve entities." on Faces to replace with |

## Delete face
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | Delete faces, Type Heal / Cap / Leave open (Heal), ☑ Delete fillet faces | same | ✅ |
| 2 | Failure | "Could not delete selected faces." (live) | "Delete face failed (…): <kernel>", cap texts | 🔧 |
| 3 | Every face picked | live: OK, the part disappears, no message | "Cannot delete every face of a part" | 🔧 |
| 4 | Surface picked first | Type switches to Leave open (deleteFaceEditLogic) | stays Heal | 🔧 L10 |
| 5 | Delete fillet faces | fillets of any origin (imported parts) | only fillets our Fillet feature named | 🔧 L11: fillets recognised from the geometry (cylinder / torus / sphere / B-spline face tangent to a deleted face and to another), imported parts too; Heal without them fails "Could not delete selected faces." (test with live's 7922.7433) |

## Sweep
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, defaults, Twist / Scale / Thin / Trim ends groups (dialog-sweep.txt) | same | ✅ |
| 2 | Empty path | "Select entities for sweep path." on path | same | ✅ |
| 3 | Thin, check order | wall checked before path: "Select faces or sketch regions to sweep." on wallShape | path first; own text, no field | 🔧 |
| 4 | Path not one chain | "Sweep path curves are not all connected." on path | own text, no field | 🔧 |
| 5 | Lock direction empty | "Select direction to lock." on lockDirectionQuery | own text, no field | 🔧 |
| 6 | Pitch revolutions limit | "Computed number of revolutions outside limit (between 0 and 1000). Current value is N." on pitch | no check | 🔧 |
| 7 | Failure | "Could not create valid swept body, check input." / thin "Failed to thicken swept body, check thickness." | "Sweep failed: …", own thin texts | 🔧 solid/surface; thin wall texts ✗ |
| 8 | Path opposite direction, Extend to full path | change where the sweep starts / whether it covers the path | ignored | 🔧 L12 (half toward the path's start / end; whole with Extend) |
| 9 | Thin method | sweep a sheet, thicken it, trim ends to planes | offset the wall, then sweep (scale scales the wall; Trim ends off ignored) | ✗ |
| 10 | Empty profiles texts | — | own texts | 🔧 L13: solid/thin "Select faces or sketch regions to sweep." (field), surface "Select sketch curves to sweep." (no field), disconnected path no field, empty Faces to lock = None |

## Loft
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, defaults, End conditions, Path section count 5, Connections, isocurves 10 | same | ✅ |
| 2 | Empty profiles | "Select profiles in order from start to finish." on profiles | same | ✅ |
| 3 | Guides, Path | supported | "…not supported yet" | ✗ |
| 4 | End conditions | applied, with their errors (planar profile, adjacent faces, tangency direction) | warning only | ✗ |
| 5 | Connections | every connection; "Select a point on each profile to define a connection." | first only | ✗ |
| 6 | Profile errors | "Point profiles can only exist as first or last profiles.", "Select a single sketch region or face per profile.", "Cannot use faces or regions with inner loops as profiles.", "Could not create valid profiles from selections." | own texts, no field | 🔧 ✅ L14 (no field) |
| 7 | Failure | "Could not create loft with given information. Check profile order, guide/profile intersections or end conditions." / thin "Failed to thicken lofted body, check thickness." | own texts | 🔧 |
| 8 | Trim profiles / guides, Thin Trim ends, Show isocurves | applied | ignored | ✗ |
| 9 | One profile | allowed with a closed guide / path | error | 🔧 L15 "Select at least two profiles, a closed guide curve or a closed path." (no field); match connections still ? live |

## Thicken
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, 5 mm, Thickness 2 = 0, Keep tools ☐, merge scope, Mid plane swap | same | ✅ |
| 2 | Empty | "Select surfaces or faces to thicken." on entities | same | ✅ |
| 3 | Keep tools off, some faces of a surface | the surface is kept unless all its faces are picked | deleted when any face is picked | 🔧 L16 |
| 4 | Adjacent faces | one operation: one body | each face separately | ✗ L17: one L-shaped part of 2560 mm³ (square corner); ours 2 slabs — needs the kernel's intersection join (kern); overlapping slabs of one body are fused |
| 5 | Auto Add from a surface face | only faces of solids count (thicken.fs) | surface faces count (opens on Add) | 🔧 L18 (surface face stays New); a user-picked Add / Remove sets the flip (canSetBooleanFlip) |
| 6 | Errors | "Failed to thicken all selections." (likely) | "Thickness must be positive", "Thicken failed: …" | 🔧 L19 "Invalid input selections." (no field) |

## Wrap
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | Creation type Solid / Surface / Split (Surface), operations for Solid, flip, Trim to target, anchors, Thickness 2.5 | same | ✅ |
| 2 | Position group | Angle / U shift / V shift in a collapsed "Position" group | always shown, no group | ✅ (fs: the std group, collapsed) |
| 3 | Filters | Tools planar only; Target cylindrical or conical only; no unwrap | wider, plus unwrap | 🔧 L20 (no unwrap; forced picks fail with live's texts) |
| 4 | Errors | "Select planar faces or sketch regions to wrap." (source), "Select a cylindrical or conical face to wrap around." (destination), "Source entities must all lie on the same plane.", "Select anchor points.", auto-anchor text, "Failed to imprint edges onto destination face.", ambiguous anchor | own texts, no fields | 🔧 (unwrap keeps its own texts) |
| 5 | Dialog dump | — | — | 🔧 L21 Position group collapsed (FS compiler lacks the group, dialog comparison excludes wrap) |

## Rib
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, Thickness 2.5 mm, Parallel, Opposite direction ☑, Draft ☐ 3°, Extend ☐, Merge ribs ☑ | same | ✅ |
| 2 | Empty profiles | "Select sketch profiles to create the ribs." on profiles | same | ✅ |
| 3 | Empty Parts | "Select parts where the rib will be fitted into." | all solids used | 🔧 L22 (no field); older ribs keep using every part |
| 4 | Parts / direction auto-fill | edit logic fills Parts and picks the side | none | 🔧 L22 Parts filled with the studio's parts on open |
| 5 | Closed profile, Parallel | "Only open sketch profiles can be used when rib direction is parallel to sketch plane." | no check | 🔧 ? L23 |
| 6 | Draft, Extend profiles to part | supported | "not supported yet" / ignored | Extend 🔧 L22: parallel ribs are built as rib.fs does (swept past the part, split by it, excess removed; ends extended with the checkbox or when they sit on a face they point into), all five live rows match (3692, 3932, 4112, 4562, error). Extend with Normal to sketch plane (L24: error) and Draft stay ✗ |
| 7 | Errors | "Failed to create a rib from a selected profile.", "None of the ribs intersected a part.", "Failed to merge ribs into parts." | own texts | 🔧 |
| 8 | One rib per edge | yes (Merge ribs off: one body per edge) | one per chain | ✅ L24: one body per profile chain (live: the L rib is one body); parallel needs one straight edge 🔧; Extend profiles see row 6 |

## Offset surface
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, 5 mm (0 allowed), Create selection, flip | same | ✅ |
| 2 | Nothing resolves | "Select faces, surfaces, or sketch regions to offset from." on surfacesAndFaces | "Nothing to offset"; one missing face fails the feature | 🔧 |
| 3 | Kernel failure | — | raw kernel text | 🔧 L25 "Could not offset selected faces as requested."; L26 one surface per picked face |

## Fill
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, Guides group, Sample size 5, isocurves 10, Merge with all ☑ | same | ✅ |
| 2 | Empty edges | "Select laminar edges or curves as fill surface boundary." on edges | own text, no field | 🔧 |
| 3 | Continuity label | "Continuity [Position]" label shown (SHOW_LABEL) | no label | 🔧 SHOW_LABEL |
| 4 | Loop errors | "A boundary edge can have only one type of continuity constraint.", "Cannot create fill surface between multiple loops.", "Boundary edges selected do not form a closed loop.", "Cannot create tangency or curvature constraint for a curve selection." | own texts | 🔧 ? L27 (internal-edge variant) |
| 5 | Guides | errors and warnings ("Select guide entities." …); exact curves in Precise mode | not checked; always sampled | ✗ |
| 6 | Add when the edges are surface edges | automatic (SF7) | no | ✗ (SF7) |

## Boundary surface
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, items, seven conditions (None), magnitude 1 | same | ✅ |
| 2 | Profile arrays | driven-query rows "[None] Edge of Sketch 1", max 2 picks each | "Add Profile" UI, no cap | ✗ |
| 3 | Boundary condition label | SHOW_LABEL | no label | 🔧 SHOW_LABEL |
| 4 | Too few profiles | "Select edges to specify at least two profiles." on both arrays | own text, no field | 🔧 L28; a third profile / mid-span crossing fail with live's texts 🔧; 1U+1V 🔧 builds the sheet the U sweeps along the V (the other two sides are the profiles moved to each other's far end; live 2099) |
| 5 | Boundary conditions | applied, with their errors | warning only | ✗ |
| 6 | Failure | "Could not create boundary surface with given information. Check profile order, intersections or end conditions." | own text | 🔧 |

## Enclose
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | Result body operation, Entities, Merge with all ☐, merge scope, Keep tools ☐ | same | ✅ |
| 2 | Empty | "Select faces, parts, or planes to form a solid." | own text | 🔧 L29 Entities marked |
| 3 | No region | "Selections do not enclose a region." | own text | 🔧 ✅ L30 |
| 4 | Failure | "Failed to create a part from enclosed region." | "Enclose failed: …" | 🔧 |
| 5 | Sketch regions | allowed | filtered out | 🔧 L31 accepted; enclosed regions make one part |

## Helix
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, defaults (dialog-helix.txt) | same | ✅ |
| 2 | Axis empty | "Select an object with an axis." on axis | own text, no field | 🔧 |
| 3 | Circle empty | "Select a circle or arc." on edge | own text, no field | 🔧 |
| 4 | Height | 1e-5–500 m (0 out of range) | 0 allowed, flat spiral | 🔧 L32 "A parameter value is not valid." (any out-of-range value, all features) |
| 5 | Zero radius | error only when both are 0 ("Start or end radius should be greater than 0."), else swapped | error when the start is 0 | 🔧 L33 start 0 with an end radius makes the conical spiral; both 0 fails |
| 6 | Start / end point | must be coplanar with the axis start / end (errors on startPoint / endPoint), snapped onto the radius | moves the plane, own errors | 🔧 helix.fs texts on startPoint / endPoint (end point only checked on a face: on an axis it sets the height); a point on a face or circle is moved onto the radius |
| 7 | Computed revolutions / pitch | filled | never | 🔧 filled when the end type is End point (helix.fs) |
| 8 | Show start and end profiles | magenta circles | nothing | 🔧 magenta debug circles (#ff2bc1) and centre dots (#e224e3) at both ends while the dialog is open (helix.fs drawConstructionEnds) |
| 9 | Preselection, start angle 0 | type from the preselection; canonical start angle | no; surface x direction | 🔧 L34 the preselection picks the tab (face → Surface, line edge / connector / sketch line → Axis, circle → Circle). L35 won't be matched: live's start angle follows Parasolid's own surface frames (a Z or X cylinder starts at the extrude's end, a tilted one at its base), which OCCT doesn't share |

## Composite part
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | Entities, Closed ☐; closed consumes members | same | ✅ |
| 2 | Empty | "Select entities to create a composite part." (likely) | own text, no field | 🔧 L36 Entities marked |
| 3 | Overlapping composites | "Cannot create a closed composite part that partially overlaps another composite part." (and open) | no check | 🔧 L36 a part of a closed composite in another closed one fails with live's text; a whole composite nests |

## Mate connector (Part Studio)
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields, empty origin | order, labels; "Cannot resolve entities." on Origin entity | same | ✅ |
| 2 | Empty owner | "Cannot determine owner of mate connector." on Select owner entity | no error | 🔧 L37 (owner inferred from the origin's part or the only part, as the dialog does) |
| 3 | Owner / attachment auto-fill | from the origin entity or the only part (connectorEditLogic) | none | 🔧 L37 |
| 4 | To selection, empty | "Select an entity for the mate connector attachment." on Attach to | no check | ✅ (already in refgeom.rs; the doc was stale) |
| 5 | Owner / attachment at regen | the connector follows them | not stored | 🔧 the owner part is kept with the connector and a Transform that moves the part moves it (attachment to a selection still follows the owner, not the selected face) |
| 6 | Move vs Reorient secondary axis | translation in the reoriented frame (likely) | translated first | 🔧 L38 order: flip/reorient, move, rotate; "+Y" = X to −Y |
| 7 | Between entities / Realign errors | built-in texts | own texts | 🔧 L39 "Failed to resolve mate connector coordinate system."; empty Realign axis = OK |

## Mirror / patterns: Face and Feature modes, Curve pattern
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | Face / Feature labels, no operation tabs, Reapply only for Feature (☐), defaults | same | ✅ |
| 2 | Empty Face mode | "Select faces to pattern." / "…mirror." on faces, before the direction | same | ✅ |
| 3 | Empty Feature mode | "Select features to pattern." / "…mirror." on Features, before the direction / plane | direction first, then "Select features to pattern" | 🔧 |
| 4 | Unresolvable plane / direction / axis | "Select a mirror plane." / "Select a pattern direction." / "Select a pattern axis." on the field | "…could not be resolved" | 🔧 |
| 5 | Linear second direction empty | error only when its count > 1: "Select a pattern direction." on directionTwo | always, own text | 🔧 |
| 6 | Reapply failures | "Could not pattern selected features." when all fail; partial failures silent | first failure's text; partial warning; geometric fallback | 🔧 texts and silent partial failures; no fallback to the geometry: a failing Reapply reports as is |
| 7 | Sketch in a feature pattern | re-run, info "Sketch dimensions and constraints using the Origin or the default planes as reference will not be reapplied. Learn more" | skipped | 🔧 L41 the sketch is copied to every instance (with or without Reapply; no part), Reapply adds the info |
| 8 | Face pattern failures | "Failed to create pattern, check input.", "Pattern could not be created on the same part.", mirror "Failed to create face mirror, check input." | own texts; off-part instances become parts | 🔧 texts; L42 any face-pattern instance off the part fails "Pattern could not be created on the same part." (no part made); L40 a Feature pattern (Reapply off) skips one off-part instance and fails when all are off (text pending from live, ours the same) |
| 9 | Feature selection in Faces to pattern | a feature-list click adds its faces | not supported | ✅ L44 (live adds nothing either; Hole not verified) |
| 10 | Curve pattern | selection checked first; "Select a path to pattern along." on edges; "Edges do not form a continuous path."; distance-too-large text on distance; closed-path rule; start-point info | own texts, no fields | 🔧 incl. the start point: an open path starts at its end nearer the seed, a closed one at its point nearest the seed (path.fs), and a start outside the seed's box gives "For best results ensure that the path start point and pattern seed are touching." ("…the path and pattern seed…" when closed) |
| 11 | Limits and infos | "Cannot have more than 2500 instances in a pattern.", parallel directions info, skip-instance infos | missing | 🔧 2500 limit (linear), parallel info, skip infos "Original pattern instance will not be skipped." / "Index is outside patterned instance count." (linear, circular, curve) |
| 12 | Feature pattern of a wall flush with the part (Reapply off) | live "psb (10-23) flush wall pattern": a wall exactly on the end face fails "Could not create all instances…" from 3 instances; with 2 the end face moves by the distance (19700 mm³); an inward wall that adds nothing fails "Selected features do not create any geometry…"; any overhang fails at 2 | flush walls patterned without error | 🔧 a feature whose faces on the part are only caps across the pattern direction: ≥3 instances fail (ghosts shown), 2 move the caps (face offset); test `feature_pattern_of_a_flush_wall_as_live` |

## Hole
| # | Area | Onshape | Ours | Status |
|---|---|---|---|---|
| 1 | Fields | order, labels, enum texts, defaults, bounds, visibility (dialog-hole.txt) | same | ✅ |
| 2 | Standard tables | holetables.gen.fs (already in crates/featurescript/std) | our own close values; missing sizes, pipe taps, PEM; labels like "1/64 (0.0156)" | 🔧 L58 read from holetables.gen.fs: every size, pipe tap and PEM entry, in live's order and with its keys ("1.00 mm (Coarse)", "1/4 (0.25)" before "E (0.25)"); paths saved with a bare pitch still resolve |
| 3 | Tap drill default | 5 mm / 0.25 in | 4.2 mm / 0.201 in | 🔧 L58 values from the FS table |
| 4 | Check order | counterbore, countersink, tolerances, tap drill, end, start, then "No hole points selected." | points first | 🔧 |
| 5 | Counterbore / countersink errors | "The counterbore diameter must be larger than the hole diameter." (marks both), "The counterbore depth must be smaller than the hole depth.", "The countersink diameter must be larger than the hole diameter."; no countersink-depth error | own texts, no fields; countersink depth error | 🔧 texts and fields; the countersink-depth error stays ? L47 |
| 6 | Up to entity / selected plane empty | "Select an entity or mate connector to terminate the hole." / "Select a start entity." on the field | own texts, late | 🔧 |
| 7 | Merge scope | auto-filled with the parts the hole reaches, direction flipped toward them; empty: "No target parts selected for hole." | empty = every solid | 🔧 L56 empty scope: "No target parts selected for hole." on the scope; L57 a new Hole's scope is the parts it reaches and follows the depth until set by hand; holes saved before (ScopeRule 4) keep cutting every part. holeScopeFlipHeuristics: more parts behind the locations than in front flips a new hole (until the flip is set by hand), and a flip that reaches nothing clears the scope (live L57) |
| 8 | Misses | "None of the holes intersected a part." (scope); warning "Some selections did not produce holes."; blind hole that misses must fail | own texts; silent; blind miss succeeds | 🔧 |
| 9 | Tapped hole cut | Diameter, tap drill only with a fastener fit | tap drill always | 🔧 L51 (+ fit info bubble) |
| 10 | Up to next / entity + Offset from tip | the tip ends at entity − offset | the shaft ends there (one tip too deep) | 🔧 L61 apex at the offset, Depth shows 4.092 |
| 11 | Depth / Tapped depth / Tap clearance | kept consistent by edit logic; inconsistency warning | independent | 🔧 L49/L50 linked in the dialog (no mismatch warning, as live) |
| 12 | Infos | overridden parameters, fastener fit < 2 parts, tapped depth readjusted, too many locations (500, on locations) | missing / own text | 🔧 500-location text; L52 "Parameters have been overridden. Check input." when Diameter or Tap drill differ from the standard (not for counterbore / countersink); "Tapped depth readjusted. …" (tapped depth past the hole's depth in the part) and "Counterbore and countersink values are non standard for tapered holes."; the inconsistent-depth warning stays off as live L49/L50 |
