## 4. Assemblies

Scope: the Assembly tab (instances, mates, relations, assembly features, positioning, in-context design, views/positions/states, BOM, interference). Simulation pages (Assembly/simulation, asynchronous_simulation, modal_simulation) are out of scope. Help base URL: `https://cad.onshape.com/help/Content/Assembly/<page>.htm` (abbreviated below as `A/<page>`). Assembly features are not FeatureScript features: there is no FS precondition, so the "FS id" column in parameter tables is `—` and the UI label from the help page is authoritative. Enum names that do exist in the FS standard library (`MateDOFType`, `MateConnectorAxisType`, `EntityInferenceType`, `OriginCreationType`, `RotationType`) and the error strings in `errorstringenum.gen.fs` are cited where they apply.

Conventions used in this section:
- **Connector frame**: a mate connector (MC) is a right-handed frame `F = (o, x, y, z)`. `z` is the "primary axis" and `x` the "secondary axis".
- In a mate, **A** is the first MC selected and **B** the second. The help says the offset "is relative to the Mate connector selected first", and that mate values/limits are "measured from the second selected Mate connector to the first, along the coordinate system". So values are A's pose expressed in B's frame.
- DOF names follow `MateDOFType` (matedoftype.gen.fs): `Tx`/`Ty`/`Tz` ("Translational X/Y/Z"), `Rz` ("Rotational Z"), `Ryp` ("Rotational Y'"), `Rzp` ("Rotational Z'"). Ryp and Rzp are rotations about axes already transformed by the previous rotation, which makes the Ball mate's 3 rotations a Z-Y'-Z' Euler sequence (inferred from the enum doc comment).

---

### AS-01 Assembly tab and assembly data model
- **Purpose:** A document tab holding a hierarchical structure of part, sketch, surface and subassembly *instances*, plus the mates, relations and assembly features that set their degrees of freedom and positions.
- **Sources:** A/assembly; A/instances_list; Home__glossary ("Instance: a part, sketch, surface, or subassembly used in an Assembly").
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - A document can hold any number of Assembly tabs. New documents get one empty Part Studio and one empty Assembly. The last remaining tab cannot be deleted.
  - An Assembly can instance parts and Part Studios from any Part Studio, other Assemblies (as subassemblies), and items from other documents. Other documents must be referenced at a **version**; same-document workspace references update live.
  - Each instance stores: a source reference (document, version or workspace, element, part id, configuration), a world transform (rigid), a name `<Name> <n>` (n = per-source instance counter), visibility, suppression (static or dynamic expression), fixed flag, and folder membership.
  - Tab context menu: Delete, Open in new browser tab, Rename, Properties (metadata), Duplicate (copy the tab in the same document, keeping references), Copy to clipboard / Paste tab into another document (the referenced Part Studios/Assemblies are pasted too and no references to the original are kept), Create Drawing of X, Select as document thumbnail, Move to document (creates a new document; links back if the tab is referenced), Export, Release, Create task.
  - Assembly toolbar: Insert, the mate types, Mate connector, Group, Relations, Replicate, Linear/Circular pattern, Mirror, Snap mode, Show mates, Create Part Studio in context, Update all references, Display states. The toolbar is customizable; `s` opens the shortcut toolbar and `alt+c` opens tool search.
  - Basic workflow: insert instances, fix one foundational instance (or fasten it to the Origin), create mate connectors and mates, then relations.
  - The assembly has an **Origin** that can be selected as a mate target ("You can mate entities to the Origin").
  - Right-panel tools on an assembly: BOM, Configuration, Variable table, Exploded views, Named positions.
- **Edge cases & errors:** `ASSEMBLY_INSERT_WILL_CAUSE_CYCLES` ("The instance could not be inserted as it would create a cyclic relationship"). "Assemblies cannot insert other assemblies as a version reference if they have the same element ID." `ASSEMBLY_CONTAINS_MISSING_PART_DATA`. `ASSEMBLY_UNKNOWN_SOLVE_ERROR` ("Failed to solve"). `ASSEMBLY_FEATURE_FAILED_TO_RESOLVE_ALL_INSTANCES`.
- **OCCT mapping:** OCCT supplies geometry only. Model the assembly tree in the app; parts are `TopoDS_Shape` placed with `TopLoc_Location` (`gp_Trsf`). For persistence, interchange and STEP, mirror the tree into an XCAF document (`XCAFDoc_ShapeTool::AddShape(..., makeAssembly=true)`, `AddComponent(assy, proto, TopLoc_Location)`, `TDataStd_Name`); see AS-62.
- **Priority:** P0

### AS-02 Instances list (instances + Mate Features list)
- **Purpose:** The left panel listing every instance, group, mate connector, mate, relation and assembly feature, with context menus, filtering and folders.
- **Sources:** A/instances_list.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Two regions: **Instances** (parts, subassemblies, patterns, mirrors, items, folders) and **Mate Features** (mate connectors, mates, groups, relations, replicates, in creation order). Mates are solved simultaneously, so their order has no effect on the solve. Users can drag to reorder for readability.
  - Instance names are `PartName <k>`. Renaming a part in its Part Studio propagates to the assembly. Implicit MCs are listed under the first mate that uses them. MCs defined on a part in its Part Studio appear in a dropdown under that instance.
  - Clicking geometry scrolls to and highlights the list row. "Go to item in list" appears in the context menu if the row is collapsed. Clicking the assembly name selects the whole assembly.
  - **Filter box** (substring match) plus colon commands, which can be combined (e.g. `:mate :errors`, `:part :hidden`): `:part`, `:assembly`, `:mate`, `:matelimits`, `:item` (non-geometric), `:folder`, `:errors`, `:warnings`, `:suppressed`, `:hidden`, `:shown`, each optionally followed by a name.
  - Optional per-row properties (Part number, Revision, State), toggled with "Show instance list properties" or a user preference.
  - **Folders** in both regions: New folder (empty, or from a selection, placed at the first selected item), nested folders, drag in/out, item count in brackets. Folder menu: Rename, Hide/Show, Hide other instances (or mates), Hide/Show all, Isolate, Make transparent, Suppress, Dynamic suppression, Add selection to folder, Expand/Collapse, Delete (deletes the contents), Unpack folder. Mate folders cannot leave the Mate Features list.
  - Hovering a mate or MC shows read-only info (offset, limits). Double-click opens the editor.
  - Errors show a red icon on the row and on its parent rows, and hovering shows the message. A panel toggle collapses the list.
  - Icons: assembly, instance, imported/derived, configured part, configured assembly, fastened-to-origin, mate limit indicator, fixed, "subassembly has a fixed instance", has-DOF triad icon, rigid assembly, configured rigid assembly, lock/follow-position, released, reference-update states (AS-52), in-context primary/active/previous primary, visibility eye, tessellation setting, error, dynamic suppression expression.
  - Per-instance context menu (full list): Properties, Instance properties (e.g. Reference designator), Edit (rigid Part Studio instance only), Show/Hide instance list properties, Show/Hide center of mass, Rename, Unfix all, Add exploded view, Release, Hide/Show, Hide other instances, Hide all instances, Isolate, Make transparent, Suppress, Dynamic suppression, Fix/Unfix, Use best available tessellation / Use automatic tessellation setting, Check interference, Replace instances, Edit in context, Add mate connector to instance origin, Switch to [Part Studio], Copy [Part], Add to named positions, Lock/follow position to, Change configuration, Move to new subassembly, Dissolve subassembly, Change to version, Export, Where used, Revision history, Create task, Add comment, Zoom to selection, Create drawing of [Part], Expand/Collapse, Add selection to folder, Create empty subassembly, Delete.
- **Edge cases & errors:** Folder creation is desktop-only (mobile can view). A subassembly containing folders cannot be dissolved.
- **OCCT mapping:** App UI/data model. Tessellation quality maps to `BRepMesh_IncrementalMesh` deflection. The assembly may use a coarser mesh than the Part Studio setting but never a finer one.
- **Priority:** P0

### AS-03 Insert parts and assemblies
- **Purpose:** Add instances of parts, sketches, surfaces, composite parts, meshes, whole Part Studios, or assemblies to the active assembly.
- **Sources:** A/insert_parts_and_assemblies. Shortcut `i`.
- **Selection inputs:** An item in the Insert dialog browser (Part Studio, a part/sketch/surface/curve within it, or an Assembly). Multiple items can be inserted per session; an "Inserted" count is shown.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| — | Source | tabs | Current document / Other documents / Standard content | The last choice is remembered |
| — | Element type | tabs | Part Studios / Assemblies | |
| — | Entity filters | toggles | sketches, surfaces, curves, parts (filtering parts or surfaces also shows meshes and mixed models) | Part Studio view |
| — | Search | text | partial and multi-word; punctuation allowed; property filters (incl. custom properties) | |
| — | View released items | filter | inserts a specific revision | if release management is used |
| — | Configuration inputs | per input | default config; "Set to last viewed configuration" / "Reset to default"; Generate | configured sources only |
| — | Insert entire Part Studio as rigid | toggle | off | then pick the Part Studio |
| — | Version | version graph / Create version | | other documents |

- **Behavior / semantics:**
  - Clicking a part inserts that part. Clicking a Part Studio inserts all of its current parts as separate instances. Clicking an assembly inserts it as a subassembly instance.
  - Placement: while the cursor is in the dialog, the first instance is placed with the Part Studio origin on the assembly origin, and later instances of the same item at a slight offset. With the cursor in the graphics area, the item follows the cursor and a click places it there. Placed instances are free (not fixed).
  - Other documents can be inserted only at a version. If the document has no version, the user is prompted to create one. The link icon turns blue when a newer version exists (AS-52).
  - With Snap mode on and explicit MCs on the inserted item, dragging onto a target MC and accepting creates a **Fastened** mate. `Ctrl` cycles the source MC (AS-16). Without explicit MCs, a normal free drag is used.
  - Copy/paste path: copy a Part in a Part Studio's Parts list (`Ctrl+C`), then Paste Part (`Ctrl+V`) in an Assembly of the same document.
  - Sketches, once inserted, can carry MCs and be mated.
  - MCs that are part-patterned in the Part Studio all come along if the part is their owner entity.
  - Deleting an instance deletes its dependent MCs, mates and relations. Groups are the exception and are not deleted (AS-11).
- **Edge cases & errors:** `ASSEMBLY_INSERT_FAILED`. `ASSEMBLY_INSERT_WILL_CAUSE_CYCLES` (e.g. inserting an assembly into itself or into its own descendant). A version reference to an assembly with the same element ID is not allowed.
- **OCCT mapping:** No geometry operation. Instance = shared `TopoDS_Shape` plus `TopLoc_Location`. Reuse tessellation across instances of the same source and configuration. Cycle check by DFS on the element reference graph.
- **Priority:** P0

### AS-04 Rigid Part Studio instance (insert as rigid, Edit)
- **Purpose:** Insert a whole Part Studio as one rigid unit that auto-updates, including newly added parts.
- **Sources:** A/insert_parts_and_assemblies ("Editing an inserted rigid Part Studio"); A/instances_list (Edit).
- **Selection inputs:** A Part Studio (with the "rigid" toggle on).
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| — | Parts / Sketches / Surfaces include toggles | toggles | include or exclude each class | Edit Part Studio instance dialog |
| — | Configuration | dropdown | Part Studio configurations | |

- **Behavior / semantics:** Parts keep their Part Studio relative positions and move as one body. Parts added later appear automatically. Edit (context menu) opens the "Edit Part Studio instance" dialog.
- **Edge cases & errors:** Behaves like a group for the solver (one rigid body).
- **OCCT mapping:** App model. One solver body containing N shapes.
- **Priority:** P1

### AS-05 Copy / paste and duplicate instances
- **Purpose:** Duplicate instances (with their MCs and mates) inside or across assemblies.
- **Sources:** A/mates ("You can copy and paste entities that have been mated"); A/instances_list (Copy [Part]).
- **Selection inputs:** One or more instances.
- **Parameters:** none.
- **Behavior / semantics:**
  - Context menu "Copy items", then "Paste" at the right-click location; `Ctrl+C`/`Ctrl+V` also work.
  - Pasted entities land where the mouse is clicked.
  - MCs and mates among the copied set are duplicated in the list (help: "the entities, Mate connectors, and Mates are also duplicated"). Whether mates to non-copied instances are duplicated is not documented (inferred: only mates fully inside the copied set).
- **Edge cases & errors:** Copying or moving that would modify a linked (immutable) document fails.
- **OCCT mapping:** App model.
- **Priority:** P1

### AS-06 Fix / Unfix / Unfix all
- **Purpose:** Lock an instance's transform in this assembly so that it acts as ground.
- **Sources:** A/instances_list (Fix); A/mates; A/insert_parts_and_assemblies.
- **Selection inputs:** Instances (context menu in the list or graphics area).
- **Parameters:** none.
- **Behavior / semantics:**
  - A fixed instance has 0 DOF and cannot be dragged: the triad does not appear and a drag shows a "cannot move" cue.
  - Fix is **local to this assembly**. It does not carry over when the assembly is inserted elsewhere, and neither does a mate to the Origin ("Neither of these carries over when the assembly is inserted into another assembly"). Only one foundational component should be grounded this way.
  - Icons: fixed instance; "subassembly has a fixed part".
  - Unfix all (assembly or subassembly menu) unfixes everything inside the selected subassemblies, folders, parametric Part Studios, patterns and replicates.
  - Rigid rule: when all instances have zero DOF and at least one is fixed, the assembly is **rigid** (rigid icon). A rigid subassembly is solved as one body in its parent.
- **Edge cases & errors:** `MATE_BETWEEN_FIXED_OCCURRENCES` ("Mate is between fixed instances"). `ASSEMBLY_CANNOT_TRANSFORM_FIXED_OCCURRENCE`.
- **OCCT mapping:** Solver: remove the body's 6 variables (AS-61).
- **Priority:** P0

### AS-07 Hide / Show instances and mates
- **Purpose:** Visibility control for instances, MCs and mates.
- **Sources:** A/instances_list; A/assembly_mate_connector; A/show_mates_mode. Shortcuts: `y` hides the selected item (in-context help), `k` toggles MCs, `j` toggles mates.
- **Selection inputs:** Instances, folders, mates, MCs.
- **Parameters:** none.
- **Behavior / semantics:**
  - Commands: Hide/Show (eye icon on hover), Hide other instances, Hide all instances, Show all.
  - For MCs: Hide, Hide other Mate connectors, Show all Mate connectors.
  - MC visibility is stored separately per tab (Part Studio and Assembly).
  - Hidden instances still participate in the solve (inferred: visibility only affects display).
  - Group context menu: Hide instances / Show instances.
- **Edge cases & errors:** —
- **OCCT mapping:** Renderer only.
- **Priority:** P0

### AS-08 Suppress / Unsuppress and dynamic suppression
- **Purpose:** Remove instances, mates, MCs or patterns from the model without deleting them.
- **Sources:** A/instances_list (Suppress, Using dynamic suppression); A/mates.
- **Selection inputs:** Instances, subassemblies, Part Studio instances, items, MCs, mates, folders.
- **Parameters:** Dynamic suppression: an expression (e.g. `#depth >= 0.5 in`). When true the item is suppressed. Configurable.
- **Behavior / semantics:**
  - Suppressing an instance makes all of its mates inactive (gray indicator).
  - Suppressing a pattern or replicate **seed** makes all pattern/replicate instances inactive.
  - Suppressed mates stay editable. Suppressed rows show strikethrough.
  - The expression icon opens the editor. Context menu: Edit dynamic suppression > Edit expression / Remove expression.
- **Edge cases & errors:** `MATE_OCCURRENCE_SUPPRESSED`, `MATECONNECTOR_OCCURRENCE_SUPPRESSED`, `RELATION_MATE_IS_SUPPRESSED`, `EXPLODE_STEP_INSTANCE_IS_SUPPRESSED`, `ASSEMBLY_ANIMATE_MATE_SUPPRESSED`, `ASSEMBLY_REPLACE_SUPPRESSED_FORBIDDEN`.
- **OCCT mapping:** App model. Exclude suppressed instances from the solver graph and the BOM.
- **Priority:** P0

### AS-09 Rename instance / instance properties
- **Purpose:** Rename list items and edit metadata.
- **Sources:** A/instances_list; Home__keyboard_shortcuts (`shift+n` renames the selected instance or feature).
- **Selection inputs:** One instance, mate, MC, folder or feature.
- **Parameters:** Name (text). Instance properties (e.g. Reference designator). Part/assembly Properties (part number, revision, description, material, and so on, shared with the source).
- **Behavior / semantics:** Renaming a mate or feature is local. Part properties edited from the assembly write back to the part source (unless it is read-only: released, versioned or linked).
- **Edge cases & errors:** Linked or versioned sources are read-only.
- **OCCT mapping:** App model.
- **Priority:** P1

### AS-10 Isolate / Make transparent
- **Purpose:** Visual focus tools: keep the selection opaque and fade the rest (Isolate), or fade the selection (Make transparent).
- **Sources:** View__isolate (`shift+i`); View__make_transparent (`shift+t`); A/instances_list; A/mates.
- **Selection inputs:** Instances, points, edges, faces, MCs, folders, groups, mates.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| — | Select transparent geometry | bool | persisted across sessions | allows picking or measuring faded geometry |
| — | Transparency (show/hide internal geometry) slider | slider | left = outer edges only ... right = fully invisible | persisted |
| — | Component controls: Expand by distance / connectivity | radio + slider | connectivity is assembly-only | grows the set by proximity, or by mate-graph distance |

- **Behavior / semantics:**
  - Hovering and pressing the shortcut adds items cumulatively. Esc exits.
  - Isolate on a mate isolates the mated instances.
  - Connectivity expansion walks the mate graph breadth-first from the selection.
- **Edge cases & errors:** —
- **OCCT mapping:** Renderer. Distance expansion: `Bnd_Box` distances, or `BRepExtrema_DistShapeShape` for precise values.
- **Priority:** P1

### AS-11 Delete instance (cascade)
- **Purpose:** Remove instances and their dependents.
- **Sources:** A/insert_parts_and_assemblies; A/group.
- **Selection inputs:** Instances.
- **Parameters:** none.
- **Behavior / semantics:**
  - Deleting an instance or feature deletes its MCs, mates and relations. Groups are *not* deleted: the member is removed from the group, and the group is deleted only when its last member goes.
  - Deleting a folder deletes its contents.
- **Edge cases & errors:** `RELATION_MATE_DOES_NOT_EXIST` can appear for relations whose mates are gone (inferred: when not cascaded).
- **OCCT mapping:** App model (dependency graph).
- **Priority:** P0

### AS-12 Replace instance
- **Purpose:** Swap one or more instances for a different component and re-apply their mates.
- **Sources:** A/replace_instance.
- **Selection inputs:** Instances to replace (part, surface, sketch or subassembly, multi-select). "Select replacement component" opens an Insert-like browser (current or other document, search and filters). The browser closes on pick.
- **Parameters:**

| FS id | UI label | Type | Default | Notes |
|---|---|---|---|---|
| — | Instances | instance list | preselection | |
| — | Select replacement component | item ref | — | |
| — | Replace all instances | bool | off | replaces every instance of the same source |

- **Behavior / semantics:** Mates are re-applied by mapping each old MC to the corresponding geometry or MC on the replacement (persistent-id matching; best effort for "slightly different / derivative" parts). Mates that cannot be mapped fail (red).
- **Edge cases & errors:** `ASSEMBLY_REPLACE_NO_REPLACER`, `CANNOT_REPLACE_IN_SUBASSEMBLIES`, `ASSEMBLY_REPLACE_INVALID_SEED_INSTANCE` ("Replace operation should be initiated by selecting pattern seed"), `ASSEMBLY_REPLACE_INVALID_SEED_TYPE` ("Only instances of solid, not flattened parts may be replaced"), `ASSEMBLY_REPLACE_SUPPRESSED_FORBIDDEN`, `ASSEMBLY_REPLACE_WILL_CAUSE_CYCLES`.
- **OCCT mapping:** Topological matching of implicit MC references to new topology. Use the app's persistent naming. Fallback: geometric matching via `BRepAdaptor_Surface`/`BRepAdaptor_Curve` type, parameters and location.
- **Priority:** P1

### AS-13 Change configuration / Change to version of an instance
- **Purpose:** Switch a configured instance to another configuration, or a linked instance to another version.
- **Sources:** A/insert_parts_and_assemblies; A/instances_list; PartStudio__part_studio_and_assembly_configurations; A/updating_references.
- **Selection inputs:** Instance(s).
- **Parameters:** Configuration inputs of the source (list / checkbox / variable). Version (from the version graph).
- **Behavior / semantics:**
  - Hovering an instance shows its active configuration as a tooltip.
  - Changing the configuration regenerates the referenced geometry. Mates keep their MCs if the references still resolve.
  - Change to version opens the Reference manager (AS-52).
- **Edge cases & errors:** Mates fail if their referenced topology disappears.
- **OCCT mapping:** Regenerate the Part Studio with the new configuration (Part Studio pipeline) and cache per configuration.
- **Priority:** P1

### AS-14 Free drag and drag-solve
- **Purpose:** Move instances by dragging geometry. The motion respects mates and limits.
- **Sources:** Home__triad_manipulator; A/mates (video transcript: "simply click and drag on the assembly's movable instances ... it can only drag so far before it stops" at a limit).
- **Selection inputs:** Drag on an instance face.
- **Parameters:** none.
- **Behavior / semantics:**
  - An unmated, unfixed instance moves exactly with the cursor (screen-plane translation; inferred).
  - A mated instance "will try to move as directed within its degrees of freedom". Connected instances move with it. Fixed instances never move.
  - Dragging stops at mate limits.
  - "In some cases, the system may not find a solution even though one exists". The workaround is to reposition the manipulator.
  - Reset (mate menu) returns dragged instances to their home positions.
- **Edge cases & errors:** `ASSEMBLY_TRANSFORM_FAILED` ("Unable to apply transform. Instance(s) may be constrained").
- **OCCT mapping:** Custom solver with a soft drag target (AS-61). Picking via the renderer.
- **Priority:** P0

### AS-15 Triad manipulator (move / rotate with typed values)
- **Purpose:** Precise move and rotate of the selected instance(s) via an on-screen triad.
- **Sources:** Home__triad_manipulator; A/exploded_views.
- **Selection inputs:** Click an instance to show the triad (not shown for fixed instances).
- **Parameters:** A transient numeric field during a drag (distance or angle, expressions allowed).
- **Behavior / semantics:**
  - Handles: 3 axis arrows (translate along an axis), 3 plane squares (translate in a plane), 3 arc handles (rotate about an axis), and the center ball (drag it to reposition the triad without moving the part).
  - The triad center snaps to any inferred (implicit) or explicit MC. After snapping, the triad adopts that frame, and drags and typed values are relative to it.
  - While dragging, a numeric field appears. Typing a value sets the exact displacement or angle relative to the MC.
  - Context menu on the center: **Move to origin** (moves the part so its reference point is at the origin; no mate or fix is added).
  - Context menu on an axis arrow: **Align with Z** / **Anti-align with Z** (rotates the part so the chosen triad axis aligns with world ±Z).
  - Context menu on an angle arc: **Rotate 90 degrees** / **Rotate 180 degrees** about that axis.
  - Mated instances move within their DOF; a requested motion is projected onto feasible motion by the solver.
- **Edge cases & errors:** Fixed instances show no triad. No solution: try another handle or triad placement.
- **OCCT mapping:** `gp_Trsf` composition (`SetTranslation`, `SetRotation(gp_Ax1, angle)`). The rendering widget is custom (GPUI/wgpu), similar in spirit to `AIS_Manipulator` in OCCT (which can serve as a reference).
- **Priority:** P0

### AS-16 Snap mode
- **Purpose:** Create mates by dragging one MC onto another, either on insert or with existing instances.
- **Sources:** A/snap_mode (`shift+s`); A/insert_parts_and_assemblies.
- **Selection inputs:** Drag source MC (explicit MCs on the inserted part; implicit/explicit for existing instances). The target MC wakes up on hover.
- **Parameters:** Toggle (toolbar, highlighted when on).
- **Behavior / semantics:**
  - **On insert:** drag the new instance over another instance. Implicit MCs on the target wake up. `Ctrl` cycles through the inserted part's explicit MCs. Click when the desired pair is active. A **Fastened** mate is created; open it afterwards to tweak.
  - **Existing instances:** drag an instance. It turns transparent while dragged, so the target's MCs are visible. When a target MC wakes, the cursor indicates the snap. Releasing opens a Mate dialog where the type and alignment can be changed and Play animates.
  - Keys during the drag: `Ctrl` cycles the source MC; `A` and `Q` change alignment (in place of the dialog's reorient-secondary-axis button; inferred: `A` = flip primary, consistent with the global `a` shortcut "Flip primary axis"); pan, zoom and rotate stay allowed.
- **Edge cases & errors:** No explicit MCs on the inserted item gives a normal free drag.
- **OCCT mapping:** UI and solver only.
- **Priority:** P1

### AS-17 Mate connector concept (explicit vs implicit, ownership)
- **Purpose:** The local coordinate frame that mates, patterns, mirror and relations use. Mates align instances by aligning one MC on each.
- **Sources:** A/assembly_mate_connector; PartStudio__mate_connector; A/mates. Shortcuts: `Ctrl+M` (MC dialog), `k` (show/hide MCs), `Shift` (lock inference).
- **Selection inputs:** see AS-18 and AS-19.
- **Parameters:** see AS-19.
- **Behavior / semantics:**
  - **Explicit MC:** created with the Mate connector tool (in a Part Studio or an Assembly). Listed top-level in the feature list. Selectable from the list while creating a mate. Reusable by many mates.
  - **Implicit MC:** created on the fly while picking geometry in a mate (or in some Part Studio features). Listed as a child of the first mate that uses it. Cannot be picked from the list.
  - An MC used by several mates is listed once (under its first mate if implicit, or at its own top-level position if explicit).
  - **Owner:** every assembly MC belongs to exactly one instance occurrence and moves rigidly with it. A Part Studio MC has an **Owner entity** (part, sketch or curve), or is ownerless if left blank. Only owned MCs travel with the part into assemblies; part-patterned MCs travel if the part is their owner.
  - Advantages of Part Studio MCs: the origin is pickable from the list, sketch entities can be referenced, and the MC is reusable on every instance in every assembly.
  - An MC's Z axis can serve as an axis selection in features.
  - Editing MCs from inside a mate dialog (MC icon in the field) is limited to MCs that live in the current assembly. Part Studio, subassembly and linked-document MCs must be edited where they are defined.
- **Edge cases & errors:** `MATECONNECTOR_MULTIPLE_OCCURRENCES` ("cannot be defined on multiple instances"), `MATECONNECTOR_OCCURRENCE_NOT_RESOLVED`, `MATECONNECTOR_OWNER_PART_NOT_RESOLVED`, `MATECONNECTOR_QUERY_FAILED` / `_ORIGIN_FAILED` / `_AXIS_FAILED` / `_CSYS_FAILED`, `MATE_CANNOT_RESOLVE_CONNECTORS`, `COMPOSITE_MATE_CONNECTOR`, `CANNOT_COPY_MATECONNECTORS`, `CANNOT_USE_MATECONNECTORS_IN_PATTERN` (Part Studio).
- **OCCT mapping:** The frame is `gp_Ax3` (or `gp_Trsf`) stored in instance-local coordinates plus a persistent reference to the defining topology, so it re-evaluates when geometry changes. World frame = instance `gp_Trsf` * local frame.
- **Priority:** P0

### AS-18 Implicit MC inference points (hover, Shift lock, cycling)
- **Purpose:** Offer candidate frames on hovered geometry while a mate or MC selection field is active.
- **Sources:** A/assembly_mate_connector; A/mates (video transcript); entityinferencetype.gen.fs; A/snap_mode.
- **Selection inputs:** Hovered face, edge, vertex, sketch entity or mesh; the Origin.
- **Parameters:** Inference type, stored hidden as `entityInferenceType` (`EntityInferenceType`) together with `originQuery` / `secondaryOriginQuery`.
- **Behavior / semantics:** Moving over geometry wakes up its inference points. The one nearest the cursor highlights as a candidate MC (a small triad preview). Clicking a face away from any point picks its centroid. Inference set (FS enum, with frame rules from the enum docs and help):

| EntityInferenceType | Origin | Z axis | Source geometry |
|---|---|---|---|
| POINT | vertex (world axes); or the tip of a conical face | world Z; or the cone axis | vertex, cone apex. Help: vertex gives "Normal to the given face" |
| CENTROID | face centroid (also the centroid of a sketch profile) | face normal at that point | planar face, any face |
| CENTER | center of a circular or elliptical edge; center of a sphere | normal of the edge's plane; world frame for a sphere | circle/arc edge, sphere |
| MID_POINT | edge midpoint | along the edge | linear or curved edge |
| TOP_AXIS_POINT / MID_AXIS_POINT / BOTTOM_AXIS_POINT | projection of the top / middle / bottom extreme of a cylinder or revolved face onto its axis | along the axis | cylinder, cone, partial cylinder, revolved faces |
| LOOP_CENTER | center of a planar edge loop (one edge of the loop is enough) | plane normal | holes, slots, regions inside a planar face ("centers of negative space for cuts") |
| VIRTUAL_SHARP | projected intersection of two non-parallel lines joined by one tangent nonlinear edge (and the cone apex virtual sharp) | plane normal | filleted corners, cone apex |
| PART_ORIGIN, ORIGIN_X/Y/Z | instance origin | (internal, not exposed in FS) | "Add mate connector to instance origin" |

  - Help-level summary of candidates: planar face gives every vertex, arc center, edge midpoint and the face centroid, with Z parallel to the face normal. Cylindrical face gives axis points at the middle and ends. Linear edge or sketch line gives the middle and ends. Circular edge gives its center (plus middle and ends). Curve endpoint or control point is aligned with world Z. Curve gives a frame tangent at the closest end, control or mid point. Vertex gives the face normal. Cone gives top, middle, bottom and virtual sharp.
  - Selecting a planar face that has a partial cylindrical edge also offers the arc axis centroid. Hovering the partial cylindrical edge gives an MC at the axis centroid.
  - Default X axis (secondary) for an implicit MC: not documented (inferred: derived from the face or edge parameterization, or the world axis most orthogonal to Z). Users fix it with Reorient secondary axis or Realign.
  - **Shift** locks the current reference face or edge (no other entities wake), which helps in tight spaces or negative space. The point then snaps to the closest inference on the locked entity.
  - **Cycling:** `Ctrl` cycles the source MC among explicit MCs in Snap mode and Insert. Overlapping selections can use the `` ` `` "select other" list (Home shortcut).
  - Connected or attached entities cross-highlight on hover.
- **Edge cases & errors:** Mesh geometry is allowed (`AllowMeshGeometry.YES`). Sheet metal flat-pattern references are prohibited (`FLATTENED_SHEET_METAL_SKETCH_PROHIBITED`).
- **OCCT mapping:** Centroid: `BRepGProp::SurfaceProperties` (planar region centroid, including loop regions built with `BRepBuilderAPI_MakeFace` on the inner wire). Normal: `BRepLProp_SLProps` or `BRepAdaptor_Surface::D1`. Circle center: `BRepAdaptor_Curve::Circle().Location()`. Axis points: `BRepAdaptor_Surface::Cylinder()`/`Cone()` axis plus projection of the face's UV bounds (`BRepTools::UVBounds`) onto the axis. Midpoint: `GCPnts_AbscissaPoint` at half length. Cone apex: `gp_Cone::Apex()`. Virtual sharp: intersect the two lines (`IntAna2d` / `Extrema_ExtElC`). Hover picking uses the renderer's ID buffer, then a nearest-candidate search in screen space.
- **Priority:** P0

### AS-19 Mate connector feature (assembly and Part Studio)
- **Purpose:** Create an explicit MC with full placement and orientation control.
- **Sources:** A/assembly_mate_connector; FS: `mateConnector` (mateConnector.fs; extracted/features/mateConnector.txt). Shortcut `Ctrl+M`.
- **Selection inputs:** Origin entity (edge, vertex, or non-construction face, mesh allowed, max 1, or an inference point, or another MC, or the Origin). Between entity (face, max 1) when the origin type is Between entities. Primary axis entity (face or edge, max 1). Secondary axis entity (face or edge, max 1). Owner entity (solid/mesh/sheet/wire/composite body, modifiable, max 1) in Part Studios.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| originType | Origin type | `OriginCreationType` | ON_ENTITY "On entity" (default) / BETWEEN_ENTITIES "Between entities" | Between = halfway between two entities |
| originQuery | Origin entity | Query | EDGE, VERTEX, or FACE (not construction); mesh ok; 1 pick | |
| entityInferenceType | — | `EntityInferenceType` | hidden | set by hover inference |
| secondaryOriginQuery | — | Query | hidden | |
| originAdditionalQuery | Between entity | Query | FACE; 1 pick | originType == BETWEEN_ENTITIES |
| realign | Realign | bool | false | |
| primaryAxisQuery | Primary axis entity | Query | FACE or EDGE; 1 | realign. Sets Z direction |
| secondaryAxisQuery | Secondary axis entity | Query | FACE or EDGE; 1 | realign. Sets XY orientation |
| transform | Move | bool | false | |
| translationX/Y/Z | X / Y / Z translation | length | ZERO_DEFAULT_LENGTH_BOUNDS: -500 m .. 500 m, default 0 | transform |
| rotationType | Rotation axis | `RotationType` | ABOUT_X "Rotate about X" / ABOUT_Y / ABOUT_Z (default) | transform |
| rotation | Rotation angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS: -1e5 .. 1e5 deg, default 0 | transform |
| allowOwnerEntity | — | bool | true, hidden | |
| requireOwnerPart | Owner entity | bool | true | Part Studio; off = ownerless |
| ownerPart | Select owner entity | Query | BODY: solid/mesh/sheet/wire/composite, modifiable; 1 | requireOwnerPart |
| specifyNormal, nx, ny, nz | — | bool, real | hidden; NORMAL_PARAMETER_BOUNDS -1..1 | internal |
| flipPrimary | Flip primary axis | bool | false | rotates the frame 180 deg so Z is reversed |
| secondaryAxisType | Reorient secondary axis | `MateConnectorAxisType` | PLUS_X "+X" (default) / PLUS_Y "+Y" / MINUS_X "-X" / MINUS_Y "-Y" | each click rotates 90 deg about Z |
| isForSubFeature | — | bool | false, hidden | implicit MCs |

- **Behavior / semantics:**
  - Evaluation order (from mateConnector.fs / `evMateConnectorCoordSystem`):
    1. Base frame from the inference on the origin entity. For Between entities, the origin is halfway between the two entities.
    2. Realign: Z along the primary entity (face normal or edge direction), X toward the secondary entity.
    3. Flip primary.
    4. Secondary axis type: the X axis is chosen as the base +X / +Y / -X / -Y, which is a 90 deg-step rotation about Z. The enum doc says "a mate connector defined with PLUS_Y will differ from the selected coordinate system by a 90-degree rotation about that coordinate system's Z-axis".
    5. Move: translate by (X, Y, Z) in the MC's own frame, then rotate by the angle about the chosen local axis.
  - An MC can be placed on an existing MC and then moved or realigned.
  - In assemblies, the MC is owned by the instance of the origin entity. "Add mate connector to instance origin" creates one at the Part Studio origin of the instance and opens this dialog.
  - Expressions and trig functions are allowed in numeric fields.
- **Edge cases & errors:** See AS-17. A mate needs its two MCs on different instances.
- **OCCT mapping:** `gp_Ax3` construction. Translation in the local frame: `gp_Trsf::SetTransformation` composition. Between entities: midpoint of the two origins (inferred). Edge direction via `BRepAdaptor_Curve::D1`. Plane normal via `BRepAdaptor_Surface::Plane().Axis()`.
- **Priority:** P0

### AS-20 Mate feature: common dialog and workflow
- **Purpose:** Create or edit a mate (one feature per pair of instances) that sets the DOF between two MCs.
- **Sources:** A/mates (shortcut `m` opens the Fastened mate dialog; `j` shows/hides mates); each mate page.
- **Selection inputs:** **Mate connectors** field: 2 MCs (implicit or explicit, or the Origin) on two different instances. Tangent uses faces, edges or vertices instead (AS-32). Width uses up to 2 + 2 MCs (AS-33).
- **Parameters (shared):**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| — | Mate type | dropdown | Fastened / Revolute / Slider / Planar / Cylindrical / Pin slot / Ball / Parallel / Tangent / Width | changeable while editing; keeps the MCs |
| — | Mate connectors | 2 MC picks | ordered; first = A, second = B | "Reorder items" + drag handles + Done to swap |
| — | Flip primary axis | toggle (arrow icon) | off | flips Z alignment (applies to the top MC in the field, or to the bottom one if the top is constrained) |
| — | Reorient secondary axis | 4-state (icon) | 0/90/180/270 deg | rotates the alignment about Z in 90 deg steps |
| — | Offset | bool + values | off | per type, see AS-21 |
| — | Offset rotation axis + angle | axis radio + angle | — | Fastened, Slider, Revolute, Pin slot, Planar |
| — | Limits | bool + min/max per DOF | off | per type, see AS-22 |
| — | Animate (play) | button | — | previews the mate's unlimited motion, ignoring other mates |
| — | Solve | button | — | solves all mates including this one |
| — | Edit MC (icon in field) | button | — | opens the MC dialog for assembly-local MCs |
| — | Simulation connection | — | — | out of scope (simulation) |

- **Behavior / semantics:**
  - Picking the second MC moves the instances so the MCs align ("the instances move together and are positioned with the mate connectors' primary Z axis aligned"). Which instance moves depends on the DOF state: fixed or constrained instances stay put (inferred, and consistent with the flip rule "If its movement is constrained, the field's bottom mate connector flips or rotates instead").
  - Each mate type then frees specific relative DOF.
  - A heads-up display at the cursor after two picks has a checkmark that commits this mate and starts a new one with the dialog kept open.
  - While the dialog is open, the full assembly is not continuously re-solved. Use **Solve** to see the effect on everything.
  - "There is exactly one Mate between any two instances" (help tip; a best-practice statement. Onshape does allow more than one, see "Use more than one Mate if necessary").
  - Expressions and trig are allowed in numeric fields. Variables (`#var`) come from the assembly variable table (AS-58).
  - **Alignment convention (inferred, verify):** at zero offset and value, `oA = oB`; `zA = -zB` (anti-parallel, so outward face normals give face-to-face contact); `xA = xB`. Flip primary changes this to `zA = +zB` (180 deg about X). Reorient secondary composes a further rotation of k*90 deg about Z. All later equations use the "aligned target frame" `B* = B * R_flip * R_z(k*90)`.
- **Edge cases & errors:** `MATE_TWO_MATECONNECTORS_NEEDED`, `MATECONNECTORS_ON_SAME_OCCURRENCE`, `MATE_WITHIN_SAME_GROUP`, `MATE_BETWEEN_FIXED_OCCURRENCES`, `MATE_OCCURRENCE_NOT_VALID`, `MATE_INVALID_MATE`, `MATECONNECTOR_INVALID_MATE`, `MATE_OVERDEFINED`, `MATE_OVERDEFINES_ASSEMBLY`, `MATE_INCONSISTENT`, `OVERDEFINED_ASSEMBLY`, `MATE_OFFSET_INVALID_ROTATION_TYPE`.
- **OCCT mapping:** Custom constraint solver (AS-61). OCCT has no assembly or 3D constraint solver.
- **Priority:** P0

### AS-21 Mate offsets
- **Purpose:** Fixed displacement or rotation built into a mate.
- **Sources:** A/mates ("Offsetting entities..."); each mate page.
- **Selection inputs:** none.
- **Parameters:** The Offset checkbox reveals the per-type fields below. Rotation offset = axis choice (X/Y/Z, maps to `RotationType` ABOUT_X/Y/Z, inferred) plus an angle. Values may be negative. Expressions are allowed.

| Mate | Translation offsets | Rotation offset |
|---|---|---|
| Fastened | X, Y, Z (any combination) | yes |
| Revolute | Z only | yes |
| Slider | X and Y (any combination) | yes |
| Planar | Z only | yes |
| Pin slot | Z only | yes |
| Cylindrical, Ball, Parallel, Tangent, Width | not documented (none) | no |

- **Behavior / semantics:**
  - "Applying an offset should be viewed as moving the entire coordinate system. The offset is relative to the Mate connector selected first": `A' = A * Trans(ox, oy, oz) * Rot(axis, angle)`, and the mate constraints apply to A'.
  - Offsets are drawn as dashed lines between the MCs with the value and axis.
  - Dragged-distance labels in the viewport can be double-clicked and edited to place parts, but those values are not saved into the mate.
- **Edge cases & errors:** `MATE_OFFSET_INVALID_ROTATION_TYPE`.
- **OCCT mapping:** `gp_Trsf` composition in the solver.
- **Priority:** P0

### AS-22 Mate limits
- **Purpose:** Restrict a mate's free DOF to ranges.
- **Sources:** A/mates; each mate page; A/instances_list (mate limit indicator, `:matelimits` filter).
- **Selection inputs:** none.
- **Parameters:** The Limits checkbox, then per free DOF an optional Minimum and Maximum (length or angle, signed, measured from B to A in B's frame, with expressions).

| Mate | Limit fields (documented / inferred) |
|---|---|
| Revolute | Rz min/max |
| Slider | Tz min/max |
| Cylindrical | Tz min/max and Rz min/max ("minimum distances for the axis of both Mate connectors"; inferred: both DOF) |
| Pin slot | Tx min/max and Rz min/max (video: X = ±half slot length; Z rotation min=max=0 locks rotation) |
| Planar | Tx, Ty, Rz min/max (inferred from DOF) |
| Ball | one cone angle: "rotational degree of movement to indicate the maximum movement from the Z axis" (i.e. a Ry' limit) |
| Parallel | "minimum and maximum distances for the relative displacement" (inferred: Tz) |
| Fastened, Tangent, Width | none |

- **Behavior / semantics:**
  - Limits are drawn as dashed lines with end bars. A limit icon appears on the mate row, and hovering shows the values.
  - Dragging cannot pass a limit.
  - Context menu **Apply limit position** moves the instance to the min or max value.
  - Animate (AS-35) defaults its Start/End to the limits.
  - A named position fails if a DOF value is outside the limits.
- **Edge cases & errors:** `MATE_MIN_MAX_LIMIT_VIOLATION` ("Minimum mate limit should be smaller than maximum limit").
- **OCCT mapping:** Inequality constraints in the solver (AS-61).
- **Priority:** P1

### AS-23 Mate value display and direct value entry
- **Purpose:** Show the current DOF values of a mate and let the user type them in.
- **Sources:** A/mates ("Mate values..."); A/named_positions.
- **Selection inputs:** Click a mate (indicator or row).
- **Parameters:** Per free DOF, an editable value label in the viewport.
- **Behavior / semantics:**
  - Clicking a mate shows its X/Y/Z axis graphics, the offset, the limit ranges, and the current values in default units.
  - Values are measured from the **second** MC to the **first** along B's coordinate system, so swapping the selection order changes signs.
  - Available for all mates **except** Ball, Fastened, Tangent and Width.
  - Typing a value repositions the parts (not saved into the definition; named positions store values persistently).
- **Edge cases & errors:** `ASSEMBLY_MATE_VALUE_SET_FAILED` ("Value cannot be applied").
- **OCCT mapping:** Solver: compute `T = B*^{-1} * A'`, then decompose into translation components and the Rz angle (atan2 of the rotated X).
- **Priority:** P1

#### Mate types (AS-24 .. AS-33)
DOF summary (relative motion of A with respect to B; R = rotational, T = translational):

| Mate | DOF remaining | Remaining axes | DOF removed | Constraint equations (A' = A with offset, B* = aligned B frame; d = oA' - oB*) |
|---|---|---|---|---|
| Fastened | 0 | — | 6 (3T+3R) | d = 0 (3); xA'·yB* = 0, xA'·zB* = 0, yA'·zB* = 0 (3), with the branch conditions zA'·zB* > 0, xA'·xB* > 0 |
| Revolute | 1 | Rz | 5 (3T+2R) | d = 0 (3); zA'·xB* = 0, zA'·yB* = 0 (2), with zA'·zB* > 0 |
| Slider | 1 | Tz | 5 (2T+3R) | d·xB* = 0, d·yB* = 0 (2); orientation equal as in Fastened (3) |
| Cylindrical | 2 | Tz, Rz | 4 (2T+2R) | d·xB* = 0, d·yB* = 0 (2); zA'·xB* = 0, zA'·yB* = 0 (2) |
| Pin slot | 2 | Tx, Rz | 4 (2T+2R) | d·yB* = 0, d·zB* = 0 (2); zA'·xB* = 0, zA'·yB* = 0 (2) |
| Planar | 3 | Tx, Ty, Rz | 3 (1T+2R) | d·zB* = 0 (1); zA'·xB* = 0, zA'·yB* = 0 (2) |
| Ball | 3 | Rx, Ry, Rz (as Rz, Ry', Rz') | 3 (3T) | d = 0 (3) |
| Parallel | 4 | Tx, Ty, Tz, Rz | 2 (2R) | zA'·xB* = 0, zA'·yB* = 0 (2) |
| Tangent | geometry dependent (typically 5) | slide and roll along the contact | typically 1 | signed distance(surface A, surface B) = 0 with normals opposed at the contact (AS-32) |
| Width | 3 (single tab: in-plane Tx, Ty, Rz on the center plane) | — | 3 (1T+2R) | (oT - c)·n = 0; zT·u = 0, zT·v = 0 (AS-33) |

Equations are the suggested noshape formulation (inferred from the documented DOF). The DOF axes themselves come from the help pages.

### AS-24 Fastened mate
- **Purpose:** Remove all 6 relative DOF between two MCs (e.g. a bolt in a hole, welded parts).
- **Sources:** A/fastened_mate; A/mates. Shortcut `m`.
- **Selection inputs:** 2 MCs (implicit or explicit, or the Origin to fasten to ground).
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| — | Mate connectors | 2 MCs | — | A = first |
| — | Flip primary axis / Reorient secondary axis | toggles | off / 0 | AS-20 |
| — | Offset | bool | off | |
| — | X / Y / Z offset | length | 0, signed | any combination |
| — | Rotation axis + angle | X/Y/Z + angle | Z, 0 | |

- **Behavior / semantics:**
  - 0 DOF: A' frame = B* frame. Manipulator drags move both instances together.
  - Mating to the Origin grounds the instance; the "fastened to origin" icon is shown at assembly level.
  - Insert-with-snap and standard content placement create Fastened mates automatically.
  - Fastened mates are unaffected by named positions.
- **Edge cases & errors:** AS-20 errors. A redundant fastened loop gives over-defined (`MATE_OVERDEFINED`) unless consistent.
- **OCCT mapping:** Solver: collapse fastened clusters into one rigid body with union-find before the numeric solve (AS-61).
- **Priority:** P0

### AS-25 Revolute mate
- **Purpose:** Allow only rotation about the common Z axis (hinge, pivot).
- **Sources:** A/revolute_mate.
- **Selection inputs:** 2 MCs. The help says "The first Mate connector selected serves as the rotational point and the second ... as the stationary point".
- **Parameters:** As Fastened, except Offset = **Z only** (plus the rotation axis/angle option). Limits: **Rz** min/max (angle).
- **Behavior / semantics:** 1 DOF (Rz). Origins coincident, Z axes collinear. The value shown is the angle from xB* to xA' about zB*. Can be driven by Gear and Rack-and-pinion relations, and by Animate.
- **Edge cases & errors:** —
- **OCCT mapping:** Solver.
- **Priority:** P0

### AS-26 Slider mate
- **Purpose:** Allow only translation along the common Z axis (prismatic joint).
- **Sources:** A/slider_mate.
- **Selection inputs:** 2 MCs.
- **Parameters:** Offset = **X and Y** (any combination) plus the rotation option. Limits: **Tz** min/max (length; negative values for the opposite direction).
- **Behavior / semantics:** 1 DOF (Tz). Orientation locked. A's origin stays on B's Z line (shifted by the X/Y offset). Can be used in Rack-and-pinion and Linear relations.
- **Edge cases & errors:** —
- **OCCT mapping:** Solver.
- **Priority:** P0

### AS-27 Planar mate
- **Purpose:** Keep two MC XY planes coincident: free in-plane translation plus rotation about the normal.
- **Sources:** A/planar_mate.
- **Selection inputs:** 2 MCs. First = moving point, second = stationary point.
- **Parameters:** Offset = **Z only** plus the rotation option. Limits: available (help: "check Limits and supply (optional) minimum and maximum values"); inferred fields Tx, Ty, Rz.
- **Behavior / semantics:** 3 DOF (Tx, Ty, Rz). After placement the instances align at the MC origins but are not held at that location. Its rotational DOF can be used in a Gear relation. Help notes that Cylindrical/Planar/Parallel used in Gear relations need extra mates to hold the parts in place.
- **Edge cases & errors:** —
- **OCCT mapping:** Solver.
- **Priority:** P0

### AS-28 Cylindrical mate
- **Purpose:** Allow translation along and rotation about the common Z axis.
- **Sources:** A/cylindrical_mate.
- **Selection inputs:** 2 MCs. First = translational/rotational point, second = stationary point.
- **Parameters:** Offset: not documented. Limits: "supply minimum distances for the axis of both Mate connectors"; inferred Tz and Rz min/max.
- **Behavior / semantics:**
  - 2 DOF (Tz, Rz). Z axes collinear.
  - It is the **only** mate valid for the Screw relation, which couples its own Rz and Tz.
  - Its Rz can feed Gear, its Tz Linear or Rack-and-pinion (with the DOF chooser).
  - Without limits it moves freely both ways along the axis.
- **Edge cases & errors:** —
- **OCCT mapping:** Solver.
- **Priority:** P0

### AS-29 Pin slot mate
- **Purpose:** A pin that rotates about its axis while translating along a slot direction.
- **Sources:** A/pin_slot_mate; video transcript in A/cylindrical_mate.
- **Selection inputs:** 2 MCs. Reference text: "selecting the pin's Mate connector first ... the second Mate connector represents the Slot". **Conflict:** the video transcript says "The first selected point of a Pin Slot mate is the slot, and the second point is the pin" and to use Reorder items if the pin was picked first. In the noshape formulation the slot direction is the X axis of the slot MC; implement the order per the reference text and keep Reorder items available.
- **Parameters:** Offset = **Z only** plus the rotation option. Limits: **Tx** min/max (e.g. ±half the slot length) and **Rz** min/max (set both to 0 to stop rotation).
- **Behavior / semantics:** 2 DOF (Rz, Tx): the pin origin moves along the slot MC's X axis, with Z axes parallel. The indicator shows an arrow in the slot direction. A "Rolling relation" requiring one pin slot mate exists in the error enum (`ROLLING_RELATION_INVALID_MATE_TYPES`) but is not documented in help (out of scope; noted as a gap).
- **Edge cases & errors:** —
- **OCCT mapping:** Solver.
- **Priority:** P1

### AS-30 Ball mate
- **Purpose:** Spherical joint: origins coincident, free 3D rotation.
- **Sources:** A/ball_mate; matedoftype.gen.fs.
- **Selection inputs:** 2 MCs. "You can select only the center of the ball" (use the sphere CENTER inference). First = rotating point, second = stationary.
- **Parameters:** Limits: one angle field = maximum deviation of A's Z from B's Z (cone half-angle). Offset: none documented.
- **Behavior / semantics:**
  - 3 DOF (Rx, Ry, Rz), parameterized as Rz, Ry', Rz' (Z-Y'-Z' Euler, per `MateDOFType`).
  - The cone limit is `acos(zA'·zB*') <= limit` (with the alignment convention).
  - The manipulator cannot drag past the limit.
  - Mate values are not displayed or editable.
  - Not recommended as the driving mate in Animate.
- **Edge cases & errors:** The Euler parameterization is singular at Ry' = 0. In the solver use a quaternion or rotation vector; use Euler angles only for display and driving.
- **OCCT mapping:** Solver.
- **Priority:** P0

### AS-31 Parallel mate
- **Purpose:** Keep the MC Z axes parallel with all translations free.
- **Sources:** A/parallel_mate.
- **Selection inputs:** 2 MCs.
- **Parameters:** Limits: "minimum and maximum distances for the relative displacement of the Mate connectors" (inferred: along Tz). Offset: none documented.
- **Behavior / semantics:** 4 DOF (Tx, Ty, Tz, Rz). Placement touches the MCs but does not hold that position. Its Rz can be used in a Gear relation (with extra mates to hold position).
- **Edge cases & errors:** —
- **OCCT mapping:** Solver.
- **Priority:** P1

### AS-32 Tangent mate
- **Purpose:** Keep two faces, edges or vertices tangent (cam/follower, roller on a surface).
- **Sources:** A/tangent_mate.
- **Selection inputs:** First entity: face, edge or vertex on instance 1. Second entity: face, edge or vertex on instance 2. **No MCs** ("Tangent Mates do not require or accept Mate connectors").
- **Parameters:**

| FS id | UI label | Type | Default | Notes |
|---|---|---|---|---|
| — | First / second entity | face/edge/vertex | — | 1 each |
| — | Tangent propagation | bool | **on** (video: "By default, it is checked") | extends to tangent-connected faces/edges. No effect for two vertices |
| — | Flip primary axis | toggle | off | only works when both picks are faces; otherwise ignored |

- **Behavior / semantics:**
  - Maintains contact with a common tangent plane at the contact point (inferred: normals anti-parallel unless flipped).
  - DOF removed: typically 1 (inferred; not documented).
  - Supported faces are **swept/analytic** only (plane, cylinder, cone, sphere, torus, other swept faces); no generic spline faces.
- **Edge cases & errors:**
  - No offset surfaces. Does not work with any relation. No mate values. Named positions may break it because it "may rely on a parameter that has changed". Not recommended for driving Animate.
  - `TANGENT_MATE_TWO_ENTITIES_NEEDED`, `TANGENT_MATE_GEOMETRY_NOT_SUPPORTED`.
- **OCCT mapping:**
  - Closed-form distance functions per analytic pair: plane-plane, plane-cylinder (`|dist(axis, plane)| - r`), cylinder-cylinder (parallel axes: `|axis distance| - (r1 ± r2)`), sphere-plane/sphere/cylinder, cone, torus. Classify with `BRepAdaptor_Surface::GetType()` / `GeomAbs_*`.
  - With propagation, take the minimum over the face chain and track the active face (switching contact).
  - General fallback: `BRepExtrema_DistShapeShape` or `Extrema_ExtSS` for the closest points, then a contact constraint with a normal check. This is costly; custom work is needed for a robust solver Jacobian.
- **Priority:** P1

### AS-33 Width mate
- **Purpose:** Center one or two "tab" MCs symmetrically between two "width" MCs (a part centered in a U-channel or clevis).
- **Sources:** A/width_mate.
- **Selection inputs:** **Tab mate connector(s)**: 1 or 2 MCs (same or different parts). **Width mate connectors**: exactly 2 MCs (same or different parts), on parts other than the tab parts. Implicit or explicit MCs. Two MCs on the same part are allowed in either field.
- **Parameters:**

| FS id | UI label | Type | Notes |
|---|---|---|---|
| — | Tab mate connector(s) | 1..2 MCs | the item to be centered |
| — | Width mate connectors | 2 MCs | define the center plane |

- **Behavior / semantics:**
  - Center plane = the plane midway between the two width MCs' XY planes. Tab parts "translate along, and rotate about, a centered plane". With 2 tabs, "the Tab mate connectors XY planes remain symmetric to each other across this centered plane": moving one tab part moves the other symmetrically.
  - Suggested equations (inferred):
    - Center point `c = (oW1 + oW2)/2`, normal `n = zW1` (W1 and W2 assumed parallel), with in-plane basis u, v.
    - Single tab: `(oT - c)·n = 0`, `zT·u = 0`, `zT·v = 0` (3 removed; Tx, Ty, Rz in-plane remain).
    - Two tabs: `((oT1 + oT2)/2 - c)·n = 0` plus parallelism of zT1, zT2 to n (the tab spacing stays free).
- **Edge cases & errors:**
  - A tab MC and a width MC cannot be on the same part.
  - Cannot be used with relations. No mate values or limits.
  - "Show Width mate" from the list displays it.
  - Mobile can only view or edit width mates.
- **OCCT mapping:** Solver.
- **Priority:** P1

### AS-34 Mate context menu and list operations
- **Purpose:** Per-mate commands.
- **Sources:** A/mates ("Use the Mate context menu").
- **Selection inputs:** A mate row or mate indicator. `Ctrl`+click for multi-select.
- **Parameters:** none.
- **Behavior / semantics:**
  - Menu: Rename; Edit (also for suppressed mates; double-click does the same); Apply limit position; **Reset** (return dragged instances to their home position "assuming constraints don't restrict that"); **Animate** (AS-35); Hide/Show; Show all mates; Show all Mate connectors; Isolate / Exit isolate; Make transparent (slider by distance or connectivity from the mate); Suppress/Unsuppress; Clear selection; Delete.
  - Also: Edit named position driver mate (AS-54), Add selection to folder, and drag to reorder.
  - Selecting a mate, MC or relation in the viewport cross-highlights its instances and list row.
- **Edge cases & errors:** `MATE_RESET_HAD_NO_EFFECT`.
- **OCCT mapping:** App/UI. "Home position" = the stored instance transforms from the last committed solve (inferred).
- **Priority:** P1

### AS-35 Animate mate
- **Purpose:** Drive the assembly through one DOF of one mate, with all other mates and relations enforced.
- **Sources:** A/mates ("Animate").
- **Selection inputs:** A mate (context menu on the row or indicator). If the mate has more than one DOF, the user picks the DOF.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| — | Degree of freedom | choice | the mate's DOFs (`MateDOFType`) | prompted if more than 1 |
| — | Start value | length/angle | mate min limit if set | |
| — | End value | length/angle | mate max limit if set | up to 36000 deg (100 revolutions) |
| — | Steps | integer | min 2 | linear map start..end, inclusive. Playback is about 60 steps/s, so more steps play slower |
| — | Playback type | enum | Single / Reciprocate / Loop | Reciprocate goes back and forth until stopped. Loop repeats start to end until stopped |
| — | Reverse (arrow) | toggle | — | reverses the direction |
| — | Current value | read-only | — | live during play. After stop, shows where it stopped |
| — | Play / Stop | buttons | — | |

- **Behavior / semantics:**
  - Each step sets DOF = value as an equality, solves warm-started from the previous step, and renders.
  - Supports all mate types. Fastened, Tangent and Ball are not recommended as drivers.
  - Works together with Isolate, mate indicator and MC display modes.
  - The in-dialog Play (AS-20) previews the mate alone, ignoring other mates.
- **Edge cases & errors:** `ASSEMBLY_ANIMATE_MATE_START_AFTER_END` ("Animation start value must be less than or equal to the end value"), `ASSEMBLY_ANIMATE_NO_MATE`, `ASSEMBLY_ANIMATE_MATE_SUPPRESSED`.
- **OCCT mapping:** Solver loop plus a render timer. No OCCT.
- **Priority:** P1

### AS-36 Mate indicators, states and errors
- **Purpose:** Viewport glyphs per mate showing type and state.
- **Sources:** A/mates ("Mate indicators"); A/instances_list.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - One glyph per type: Fastened, Revolute, Slider, Planar, Cylindrical, Pin slot (with an arrow in the slot direction), Ball, Parallel, Tangent, Width, Group.
  - Colors: **blue/white = OK**; **gray = suppressed or inactive** (for example, an instance is suppressed); **red = problem** (failed to resolve, over-defined, inconsistent).
  - Errors also show in the list with red icons propagated to parent rows. Hovering shows the message.
  - Instance DOF icons: a triad icon = has DOF (tooltip); rigid icon = fully constrained subassembly. Non-rigid subassembly children show no DOF info.
  - Indicators have context menus (Edit, Animate, etc.) and cross-highlight.
- **Edge cases & errors:**
  - Over-defined: `MATE_OVERDEFINED`, `MATE_OVERDEFINES_ASSEMBLY`, `OVERDEFINED_ASSEMBLY`, `RELATION_OVERDEFINED`.
  - Inconsistent (no solution): `MATE_INCONSISTENT`, `RELATION_INCONSISTENT`.
  - Unresolved: `MATE_CANNOT_RESOLVE_CONNECTORS`, `MATE_OCCURRENCE_NOT_VALID`, `MATE_GROUP_OCCURRENCES_UNRESOLVED`.
- **OCCT mapping:** Renderer glyphs. States from solver diagnostics (rank/residual analysis, AS-61).
- **Priority:** P1

### AS-37 Show mates mode and mate visibility
- **Purpose:** Reveal only the mates related to the hovered or selected instance.
- **Sources:** A/show_mates_mode (`h`); A/mates (`j` toggles all mates).
- **Selection inputs:** Hover or click instances (viewport or list).
- **Parameters:** Mode toggle.
- **Behavior / semantics:**
  - Mates are hidden in the viewport by default.
  - `j` shows/hides all. The eye icon per mate toggles one. Right-click Show/Hide works on several mates, and on an instance it shows all of that instance's mates.
  - In Show mates mode, hovering an instance shows its mates and clicking keeps them visible.
  - Shows mates at all subassembly levels.
  - Clicking a mate icon cross-highlights its row. The icon's context menu offers Edit.
  - Instance context menu "Show mates".
- **Edge cases & errors:** —
- **OCCT mapping:** Renderer.
- **Priority:** P2

### AS-38 Group (rigid set) vs Fastened
- **Purpose:** Freeze the current relative positions of selected instances so they move as one rigid unit, without mates.
- **Sources:** A/group.
- **Selection inputs:** Instances: list click, viewport click, or box select; preselection allowed. A part inside a subassembly selects the whole subassembly instead. Subassemblies must be **rigid** to be grouped.
- **Parameters:** Instances (multi). No numeric parameters.
- **Behavior / semantics:**
  - Creates a Group feature in Mate Features. Members keep their relative transforms, based on instance **origins** ("positions parts based only on their origin and ignores changes in geometry").
  - The group still has 6 DOF as a whole until it is fixed or mated.
  - Changing relative positions in the source Part Studio updates the group. Best practice is to group instances from the same Part Studio, because parts from separate Part Studios can overlap after geometry changes.
  - Suppress the group, move members, then unsuppress: the group re-captures the new relative positions.
  - Hide instances / Show instances from the group context menu.
  - A group can be seen and inserted from another assembly's Insert > Assemblies, where it is listed as a single entity ("Group instance"; children cannot be acted upon individually).
  - Unlimited groups. Mates between members of the same group are rejected.
  - Patterning a group member puts the new instances in the same group.
  - Replicating groups is not supported.
  - **vs Fastened:** a Fastened mate references MC geometry, so it follows geometry changes and can carry offsets. A Group references only origins and relative transforms. The help points to Group "for purchased parts that do not change, or for parts created together in the same Part Studio".
- **Edge cases & errors:**
  - Deleting a member removes it from the group. Deleting the last member deletes the group. Deleting an instance does not delete groups.
  - `MATE_WITHIN_SAME_GROUP`, `MATE_GROUP_OCCURRENCES_UNRESOLVED`.
- **OCCT mapping:** Solver: union-find merge into one rigid body with fixed relative `gp_Trsf` per member.
- **Priority:** P0

### AS-39 Relations (common behavior)
- **Purpose:** Couple DOF of mates: Gear, Rack and pinion, Screw, Linear.
- **Sources:** A/relations.
- **Selection inputs:** Mates, picked from the viewport or the Mate Features list, including mates inside current subassemblies.
- **Parameters:** Relation type (dropdown in the dialog), the mates, a DOF per mate (auto-selected when the mate has exactly the required DOF, otherwise a secondary chooser dialog), Ratio or distance with units, Reverse direction (bool). Expressions allowed.
- **Behavior / semantics:**
  - Once a DOF is chosen it cannot be changed except by removing the mate from the dialog, changing the mate type, or deleting and recreating.
  - The coupling holds in drag, animate and solve.
  - Suggested equation form (inferred): `v2 - v2_ref = s * k * (v1 - v1_ref)`, where `v_ref` are the DOF values when the relation was created or last edited, `k` is the ratio, and `s = ±1` from Reverse plus the per-type default sign. Mates with ambiguous rotation-angle branches must be unwrapped (track continuous angle, not mod 2π).
- **Edge cases & errors:**
  - `RELATION_INVALID_RELATION`, `RELATION_INVALID_MATE`, `RELATION_OVERDEFINED`, `RELATION_INCONSISTENT`, `RELATION_SAME_OCCURRENCES` ("Mates in relation cannot involve the same instance"), `RELATION_MATE_DOES_NOT_EXIST`, `RELATION_MATE_IS_SUPPRESSED`.
  - Tangent and Width mates cannot be used in relations.
- **OCCT mapping:** Solver coupling equations (AS-61).
- **Priority:** P1

### AS-40 Gear relation
- **Purpose:** A constant angular ratio between two rotational DOF.
- **Sources:** A/gear_relation.
- **Selection inputs:** 2 mates with a rotational DOF: Revolute (exact), or Cylindrical, Planar, Parallel, Pin slot (via the DOF chooser; Pin slot is listed in the video transcript only).
- **Parameters:**

| FS id | UI label | Type | Notes |
|---|---|---|---|
| — | Mates | 2 mates (+ DOF each) | first = driving/input (inferred from the ratio definition) |
| — | Ratio | real | "teeth of the driving (input) gear divided by teeth of the driven (output) gear". Example 20/40 = 0.5: one input turn gives half an output turn |
| — | Reverse direction | bool | flips the output rotation sense |

- **Behavior / semantics:** `Δθ_out = ±ratio * Δθ_in`. Works for pulleys, clock hands and other non-gear pairs. Cylindrical/Planar/Parallel-based gears need extra mates to hold position. The default rotation sense relative to the MC Z axes is not documented (inferred: same sense; Reverse gives the opposite).
- **Edge cases & errors:** `GEAR_RELATION_INVALID_MATE_TYPES` ("Gear relations require two rotational degrees of freedom").
- **OCCT mapping:** Solver.
- **Priority:** P1

### AS-41 Rack and pinion relation
- **Purpose:** Couple a rotation with a translation.
- **Sources:** A/rack_and_pinion_relation.
- **Selection inputs:** 2 mates: a Revolute (rotational DOF) and a Slider (linear DOF) are exact. Others go through the DOF chooser.
- **Parameters:** Linear value = distance the rack travels per pinion revolution (length). Reverse (bool).
- **Behavior / semantics:** `Δx = ±(L / 2π) * Δθ`. Reverse moves both in opposite directions to each other.
- **Edge cases & errors:** `RACK_RELATION_INVALID_MATE_TYPES` ("require one translational degree of freedom and one rotational degree of freedom").
- **OCCT mapping:** Solver.
- **Priority:** P1

### AS-42 Screw relation
- **Purpose:** Couple the rotation and translation of a **single** Cylindrical mate.
- **Sources:** A/screw_relation.
- **Selection inputs:** Exactly 1 Cylindrical mate.
- **Parameters:** Distance per revolution (length; the help calls it "ratio" in the steps). Reverse direction (bool).
- **Behavior / semantics:** `Δz = ±(p / 2π) * Δθ` on the same mate (vise spindle, lead screw). Removes 1 DOF.
- **Edge cases & errors:** `SCREW_RELATION_INVALID_MATE_TYPES` ("Screw relations require one cylindrical mate").
- **OCCT mapping:** Solver.
- **Priority:** P1

### AS-43 Linear relation
- **Purpose:** A constant ratio between two linear DOF.
- **Sources:** A/linear_relation.
- **Selection inputs:** 2 mates with translational DOF. Two Sliders are exact; others go through the chooser.
- **Parameters:** Ratio (real, default 1 inferred from the transcript "change the ratio value to something other than 1"). Reverse (bool).
- **Behavior / semantics:** `Δx2 = s * ratio * Δx1`. Per the video transcript, by default "when one guide is moved, the other guide moves in the opposite direction", and **Reverse makes both move in the same direction**. (The reference text says "The first Mate will move linearly in one direction as the other Mate is moved linearly in one direction", which is ambiguous.)
- **Edge cases & errors:** `LINEAR_RELATION_INVALID_MATE_TYPES` ("Linear relations require two translational degrees of freedom").
- **OCCT mapping:** Solver.
- **Priority:** P1

### AS-44 Replicate
- **Purpose:** Copy a mated seed (e.g. a bolt) onto every matching geometry (e.g. all identical holes), creating instances plus mates.
- **Sources:** A/replicate.
- **Selection inputs:** **Instances**: seed instance(s), already mated, not grouped. **Mate to be matched**: auto-filled when the seed has a single mate, otherwise pick one from the list. **Parts to find match in**: parts, features, faces or edges, depending on the scope.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| — | Instances | instances | seed(s) | |
| — | Mate to be matched | mate | auto if single | |
| — | Match scope (face) | enum | Match faces in parts / Match faces in features / Match individual faces | shown when the seed mate was made on a face |
| — | Match scope (edge) | enum | Match edges on plane / Match edges on face / Match individual edges | shown when it was made on an edge |
| — | Parts to find match in | query | per scope | |

- **Behavior / semantics:**
  - Finds geometry identical to the seed mate's target entity within the scope. For each match it creates a seed copy and a mate equivalent to the seed mate, with the MC inferred the same way on the matching entity.
  - Creates a "Replicate" feature in the list with child mates.
  - **Dissolve Replicate** turns it into individual mates and instances.
  - Updates when the targets change (inferred).
  - Suppressing the seed makes all replicated instances inactive.
- **Edge cases & errors:**
  - The seed may have only **one external mate** (to the target).
  - `ASSEMBLY_REPLICATE_NO_VALID_TARGET`, `ASSEMBLY_REPLICATE_NO_MATCHING_TARGET`, `ASSEMBLY_REPLICATE_MULTIPLE_VALID_TARGET`, `ASSEMBLY_REPLICATE_NO_TARGET_SELECTED`, `ASSEMBLY_REPLICATE_INVALID_SEED_INSTANCE`, `ASSEMBLY_REPLICATE_INVALID_SEED_MATE`, `ASSEMBLY_REPLICATE_MISSING_SEED_MATE`.
  - Errors are shown via hover on the orange dialog title.
- **OCCT mapping:** Geometric signature matching:
  - Surface type plus parameters (`BRepAdaptor_Surface`: cylinder radius, cone angle, plane), edge type and length (`BRepAdaptor_Curve`, `GCPnts_AbscissaPoint::Length`), adjacency signature (`TopExp::MapShapesAndAncestors`), all compared with tolerance.
  - Then re-infer the same `EntityInferenceType` on each match.
  - Iterate over `TopExp_Explorer` within the scope (faces of parts, features, or planar-face edges).
- **Priority:** P1

### AS-45 Assembly linear pattern
- **Purpose:** Pattern instances along 1 to 3 directions.
- **Sources:** A/assembly_linear_pattern.
- **Selection inputs:** **Instances**: part, subassembly, sketch or surface instances. **Direction**: edge, face, or MC (implicit via the MC button; editable). Second and Third direction: same types.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| — | Instances | instances | — | |
| — | Direction | edge/face/MC | — | linear edge direction, face normal, or MC Z (inferred) |
| — | Distance | length | > 0 | spacing, or the total span if Equal spacing |
| — | Instance count | integer | min 1 (includes the seed) | |
| — | Opposite direction | toggle | off | |
| — | Equal spacing | bool | off | instances spread evenly within Distance |
| — | Centered | bool | off | the seed is the center: N instances (including the seed) in each direction |
| — | Second direction / Third direction | bool + same fields | off | grid |

- **Behavior / semantics:**
  - Instance i is placed at `T_seed * Trans(i * d * dir)`. With multiple directions, take the Cartesian product.
  - Pattern instances follow the seed's motion. Seed mates are carried (inferred: pattern instances are rigidly offset from the seed's solved pose).
  - Patterned subassemblies stay flexible and track the seed's internal positions.
  - Instances of a grouped seed join the group.
  - Suppressing the seed deactivates all instances.
  - Patterns can be moved into subassemblies.
- **Edge cases & errors:** `ASSEMBLY_PATTERN_NONPOSITIVE_LINEAR_DISTANCE`, `ASSEMBLY_PATTERN_DIRECTION_ERROR`, `ASSEMBLY_PATTERN_INVALID_SEED`, `ASSEMBLY_PATTERN_RECURSIVE_SEED` ("Pattern instances of same pattern cannot be selected as seed"), `ASSEMBLY_PATTERN_EXCEED_MAX_INSTANCE_COUNT` (total ≤ 2500), `ASSEMBLY_PATTERN_INVALID_REFERENCE_MATE_CONNECTOR`, `ASSEMBLY_PATTERN_INVALID_TYPE`.
- **OCCT mapping:** `gp_Trsf::SetTranslation`. Direction from `BRepAdaptor_Curve` (line) or `BRepAdaptor_Surface` (plane normal).
- **Priority:** P1

### AS-46 Assembly circular pattern
- **Purpose:** Pattern instances about an axis.
- **Sources:** A/assembly_circular_pattern.
- **Selection inputs:** **Instances** (as above). **Axis of pattern**: edge, face, MC (implicit or explicit), conic or cylindrical face.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| — | Axis of pattern | edge/face/MC | — | cylinder or cone axis, linear edge, MC Z |
| — | Angle | angle | > 0 | angle between instances, or the total if Equal spacing |
| — | Instance count | integer | min 1 | |
| — | Opposite direction | toggle | off | |
| — | Equal spacing | bool | off | |

- **Behavior / semantics:** Instance i is placed at `Rot(axis, i * angle) * T_seed`. Group, suppression and subassembly behavior are the same as for the linear pattern.
- **Edge cases & errors:** `ASSEMBLY_PATTERN_NONPOSITIVE_ANGLE`, `ASSEMBLY_PATTERN_AXIS_ERROR`, plus the shared pattern errors. Total ≤ 2500.
- **OCCT mapping:** `gp_Trsf::SetRotation(gp_Ax1, angle)`. Axis from `BRepAdaptor_Surface::Cylinder().Axis()` / `Cone().Axis()` or a line edge.
- **Priority:** P1

### AS-47 Assembly mirror
- **Purpose:** Mirror instances (parts, surfaces, subassemblies) across a plane, either by rigid transform (symmetric parts) or by a derived mirrored part (handed parts).
- **Sources:** A/assembly_mirror.
- **Selection inputs:** **Instances to mirror** (any combination). **Mirror plane**: plane, planar face or edge (via implicit MC), or an explicit MC. The plane is the MC's XY plane (inferred).
- **Parameters (Mirror strategy table, per instance row):**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| — | Instance | read-only | | |
| — | Source | read-only | version of the source | |
| — | Symmetry | read-only | symmetric / asymmetric | auto-detected |
| — | Mirror action | enum | **Transform** (default) / **Derived** | Derived is for handed (asymmetric) parts |
| — | Mirror reference | link | the existing mirrored part, or an "info: needs creation" marker | opens in a new tab |
| — | Placement type | enum | Auto position / Bounding box XY / Bounding box YZ / Bounding box ZX / Origin XY / Origin YZ / Origin ZX | Transform rows: the assumed local symmetry plane |
| — | Update (button) | — | — | creates the missing derived references |

- **Behavior / semantics:**
  - **Transform:** copies the instance with a *proper* rigid transform: `T' = M_world * T * S_local`, where `M_world` is the reflection in the mirror plane and `S_local` is the reflection in the part's chosen local symmetry plane (bounding-box mid-plane or origin plane per Placement type). Two reflections compose to a rotation and translation, so no geometry is mirrored. Auto position picks the best of the 6 options.
  - **Derived:** creates a Part Studio "[name]-Mirrored" (in a "Mirrored" folder) with a Derived mirror of the part, suffixing the Name property with "-Mirrored". For subassemblies it creates "[subassembly]-Mirrored". The new parts are inserted under the Mirror feature. If the source is a linked version, the mirrored part is created and auto-versioned in the linked document, and then the current document is auto-versioned. Sources without a current version get one first.
  - Mirrored subassembly motion follows the original. Mates are not recreated, so the mirror does not move independently.
  - Configurations are mirrored too (the derived configuration tables are view-only).
  - Instances can be added after creation.
  - Symmetry changes update the Symmetry column but not the Mirror action.
  - Sheet metal Derived mirror also generates the flat pattern. Mirroring a closed composite together with an active sheet metal model via Derived gives solid parts only.
- **Edge cases & errors:**
  - Mirroring under-constrained subassemblies gives limited solvability, so mirror fully constrained ones.
  - Do not transform parts inside mirrored Part Studios or assemblies.
  - Deriving many parts creates many versions.
  - Release after mirroring.
- **OCCT mapping:**
  - Reflection: `gp_Trsf::SetMirror(gp_Ax2)`. Derived mirrored geometry: `BRepBuilderAPI_Transform(shape, mirrorTrsf, copy=true)` (handedness flips orientation; OCCT handles it).
  - Symmetry detection (inferred approach): compare the shape with its reflection in the bbox mid-planes (`Bnd_Box`; volume of `BRepAlgoAPI_Common` vs original via `BRepGProp::VolumeProperties`, or a fast mesh/Hausdorff check).
- **Priority:** P1

### AS-48 Subassembly restructuring (move to new, create empty, dissolve, drag in/out)
- **Purpose:** Reorganize the hierarchy without losing mates.
- **Sources:** A/assembly ("Rearranging instances and subassemblies"); A/instances_list (Notes).
- **Selection inputs:** Instances, patterns, subassemblies in the list.
- **Parameters:** none (a confirmation dialog lists dependencies).
- **Behavior / semantics:**
  - **Move to new subassembly:** moves the selected parts or patterns, plus their dependencies (reviewed in a "Move to new subassembly" dialog), into a new Assembly tab, which is instanced in place. A new Mate features entity is created inside it (empty if there were no mates). Naming: `Assembly X <x>`.
  - **Create empty subassembly:** a new empty Assembly tab plus an instance of it in the current assembly. Whatever is built there appears live.
  - **Drag and drop** instances into or out of subassemblies and to the top level. Mates are maintained (re-parented to the lowest common assembly; inferred).
  - **Dissolve subassembly** (one or several, from the parent tab): moves the child instances and mates into the parent and leaves the tab empty (not deleted). Existing versions that reference the subassembly are unchanged.
  - Expand/Collapse selected or all.
- **Edge cases & errors:**
  - Cannot dissolve if the subassembly contains folders.
  - Moving or copying that would modify a linked (immutable) document fails.
  - `RESTRUCTURE_INVALID_TARGET`.
- **OCCT mapping:** App model operations (tree edit plus mate re-parenting with transform re-basing).
- **Priority:** P0

### AS-49 Subassembly solve behavior (flexible vs rigid, lock/follow position)
- **Purpose:** Define how subassembly internals behave inside a parent.
- **Sources:** A/assembly (subassembly and lock/follow sections); A/instances_list; A/mates (video transcript).
- **Selection inputs:** Subassembly instance (context menu "Lock/follow position to").
- **Parameters:** Lock/follow: **Lock current position** / **Named position <name>** (if any exist) / unlocked (default).
- **Behavior / semantics:**
  - By default "subassembly mates are respected by the top-level assembly. They operate the same way there as they do in their original assembly." The subassembly is **flexible**: its internal DOF can be dragged and solved in the parent, and each instance occurrence of the subassembly can sit in a different articulation (inferred).
  - Fix and mate-to-origin inside the subassembly do **not** carry into the parent. The subassembly as a whole gets 6 DOF in the parent until it is mated or fixed.
  - A subassembly with zero internal DOF and a fixed instance is **rigid** (rigid icon) and is solved as one body. This improves performance.
  - **Lock current position:** freezes the internal mate positions at their current values in the parent (lock icon). This improves performance and prevents accidental motion.
  - **Follow named position:** uses the subassembly's named position. If that position changes in the subassembly tab, the parent follows after the subassembly regenerates.
  - The lock icon on hover gives a quick lock. Clicking it again unlocks.
  - Non-rigid subassemblies and their children show no DOF icons.
  - Relations can reference mates inside current subassemblies (AS-39).
  - Show mates mode shows mates at all levels.
- **Edge cases & errors:** Mirrored subassemblies follow the original's motion (AS-47). A group can contain only rigid subassemblies.
- **OCCT mapping:** Solver. A rigid or locked subassembly is one body. A flexible subassembly is flattened into the parent's system (its bodies and mates become variables and equations, with an independent copy per occurrence).
- **Priority:** P0

### AS-50 Modeling in context (Edit in context, assembly contexts, update context)
- **Purpose:** Edit a part's Part Studio while seeing and referencing the surrounding assembly geometry (top-down design).
- **Sources:** A/modeling_in_context.
- **Selection inputs:** The primary instance (context menu "Edit in context" > existing context or "Create new context"; or double-click). References to ghosted geometry inside Part Studio features.
- **Parameters (context object, in the Part Studio "Assembly contexts" list):** name, primary instance, Follow position (current or a named position), the visible context instances, transparency slider.
- **Behavior / semantics:**
  - Opens the Part Studio with the other assembly instances drawn transparent (ghosted), placed relative to the primary instance's assembly pose. The Part Studio's own parts are opaque, in their original positions, and the edited part is highlighted. The view orientation, zoom and section view carry over in both directions.
  - **The first reference to ghosted geometry** creates an *Assembly context*: a snapshot of the geometry and positions, listed above the Feature list. Referenced entities highlight purple. Features using a context show an in-context arrow, highlighted yellow for the selected context.
  - References "are never lost or broken": they point into the frozen snapshot.
  - **Updates are never automatic.** Update context is available from the assembly (part context menu > Update context > pick a context) or the Part Studio (context three-dot menu). A blue dot notification appears when the source changed (tooltip lists up to 2 contexts "and more").
  - Multiple contexts per Part Studio (different articulations). None = context off. Rename, Change configuration of context instances, Hide/show context instances (`y` hides selected), Restore context view state.
  - **Primary instance:** shown with a solid arrow; the previous one with a dashed arrow. "Set as primary instance" re-anchors (needed if the primary is missing).
  - Exiting via the banner X ends the session without a context. Done saves. "Go to assembly" returns.
  - For a version-referenced instance, a "Select a workspace to edit" dialog appears.
  - A context persists only if something references it. New parts that do not reference the edited part are not auto-inserted.
- **Edge cases & errors:**
  - Revision references cannot be edited in context.
  - `IN_CONTEXT_INSTANCE_INVALID_TARGET`, `IN_CONTEXT_UPDATE_DELETED_ASSEMBLY`, `IN_CONTEXT_UPDATE_EMPTY_INSTANCE`, `IN_CONTEXT_UPDATE_INVALID_SOURCE`, `IN_CONTEXT_UPDATE_INVALID_TARGET`, `ASSEMBLY_CONTEXT_INVALID_REFERENCE`.
- **OCCT mapping:**
  - Snapshot = copies of the referenced instance shapes (`BRepBuilderAPI_Copy` or shared `TopoDS_Shape` plus location) stored in the Part Studio, expressed in the primary instance's frame: `inverse(T_primary) * T_other`.
  - References resolve against the snapshot's persistent ids. Update context re-captures the snapshot and regenerates.
- **Priority:** P1

### AS-51 Create Part Studio in context
- **Purpose:** Start a new Part Studio whose origin is at an assembly MC or origin, with the assembly as context.
- **Sources:** A/create_part_studio_in_context.
- **Selection inputs:** One explicit MC or the assembly Origin (sets the new Part Studio's origin and planes).
- **Parameters:** Origin MC (1).
- **Behavior / semantics:**
  - Creates and opens a new Part Studio whose default planes align to the chosen frame, with the assembly dimmed as context.
  - The first reference creates a context object.
  - Banner options: **Insert and go to assembly** (pick the new parts, insert them back; they become the primary instance with a solid arrow) or **Go to assembly** (no insert). Inserting later via Insert gives a dashed-arrow, non-primary association.
- **Edge cases & errors:** As AS-50.
- **OCCT mapping:** As AS-50. The new Part Studio world = the chosen frame.
- **Priority:** P1

### AS-52 Updating references (linked documents, Reference manager)
- **Purpose:** Track and update instances that reference versions or revisions of other documents (or of the same document).
- **Sources:** A/updating_references; A/insert_parts_and_assemblies.
- **Selection inputs:** Instance reference icons. Rows in the Reference manager.
- **Parameters:** Reference manager filters: **Update to latest** / **Selective update**, Pinned filter, Revision filter, Version graph picker. Per-row checkbox plus a target version or revision. "Update selected". Workspace choice when newer versions exist on several branches.
- **Behavior / semantics:**
  - Icon states:
    - version reference, up to date
    - blue background = newer version on its branch, in that tab with edits
    - triangle = revision reference (latest, or newer available)
    - thumbtack = pinned (excluded from Update all; an open gray circle replaces blue)
    - solid gray circle = a child has an update (navigate into it)
    - small arrow blue = subassembly contains an updatable child
    - small arrow gray = blocked (pinned, revision, or pinned child)
    - auto-version icon in history; auto-obsolete icon
  - Out-of-date state propagates up the tree.
  - **Update all** (toolbar) is recursive. It creates new versions of intermediate linked documents as needed so the top level can pull nested updates, and updates every unpinned reference, even those already up to date.
  - Revisions must be updated at the owning level first.
  - **Pin reference / Unpin** via the icon context menu. Pinned children inside subassemblies must be updated there.
  - "Update to the new document" when an instance was moved to another document.
  - Documents without write permission disable Update.
- **Edge cases & errors:** Immutable versions cannot be edited.
- **OCCT mapping:** App/PDM data model (no geometry).
- **Priority:** P1

### AS-53 Exploded views
- **Purpose:** Named sequences of explode steps (translate or rotate instance sets) with explode lines, for display and drawings.
- **Sources:** A/exploded_views.
- **Selection inputs:** In explode mode: instances (click or box select). Optional direction (face or edge), axis (face or edge), explode-line anchors (edges, vertices, faces).
- **Parameters (Explode step dialog):**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| — | Instances | instances | current selection | sketch instances not supported |
| — | Movement type | enum | Translation / Rotation | |
| — | Specify direction + entity | bool + face/edge | off | Translation |
| — | Specify axis + entity | bool + face/edge | off | Rotation |
| — | Translation distance / Rotation angle | length / angle | from the drag; negative values allowed | |
| — | Explode lines | bool | **on** | |
| — | Edges, vertices, or face for explode lines | query | centroid of all selected entities (default) | the line starts at the centroid of the chosen entity per instance |

- **Behavior / semantics:**
  - **Exploded views panel:** Add exploded view creates "Explode N" and enters explode mode (gold banner; mates, patterns and relations are disabled).
  - Drag the triad (its origin can be repositioned) and release to create a step. The selection persists for chained steps; clicking a new instance replaces the selection. Done exits.
  - Each view = an ordered list of steps with a **rollback bar**: drag it, or select it and use the ↑/↓ arrow keys. Steps below the bar are suppressed.
  - Steps can be renamed, edited, reordered by drag, and moved between views.
  - View context menu: Activate (display and zoom to fit), Rename, Show/hide explode lines (view or step), **Reset starting position** (re-base on the current assembly pose), Duplicate, Delete.
  - Explode lines (trails) follow the moved instances' anchor points from start to end. A new step inherits the previous step's line selections.
  - Configurable step fields. A configuration missing a referenced instance turns that step and view red.
  - Drawings use the full view (all steps), regardless of the rollback position.
  - Visible in Follow mode.
- **Edge cases & errors:** `ASSEMBLY_EXPLODE_STEP_DIRECTION_QUERY_FAILED`, `ASSEMBLY_EXPLOSION_DOES_NOT_EXIST`, `EXPLODE_STEP_INSTANCE_IS_SUPPRESSED`, `EXPLODED_VIEW_INVALID_STARTING_POSITION`, `FAILED_TO_CREATE_EXPLODED_VIEW`, `EXPLODE_LINES_MUST_SPECIFY_STARTING_POINT`, `EXPLODE_LINES_FAILED_TO_RESOLVE_ENTITY`, `EXPLODE_LINE_NOT_ON_EXPLODED_INSTANCE`, "Explode steps do not support instances of type Sketch".
- **OCCT mapping:**
  - Display-only transforms layered on the starting pose: `T_i = Π(step transforms that include i) * T_start,i`. Translation uses `gp_Trsf::SetTranslation` along the edge direction or face normal. Rotation uses `SetRotation(gp_Ax1)`.
  - Explode lines are polylines between the start and end anchor points (`BRepGProp` centroid). No solver involvement.
- **Priority:** P1

### AS-54 Named positions
- **Purpose:** Save and restore assembly articulations (mate DOF values plus transforms of unmated instances).
- **Sources:** A/named_positions.
- **Selection inputs:** In edit mode: drag parts. Click a mate or part to open the **Driving mate selector** (choose the mate DOFs that become table columns).
- **Parameters:** A table with one row per position (Name cell) and one column per driving mate DOF (numeric, editable). Column menu: Move column right/left, Move to start/end, Delete.
- **Behavior / semantics:**
  - Add named position enters edit mode (yellow banner, Done to exit).
  - Row menu:
    - Apply named position: sets the DOF values and solves
    - Edit named position
    - Duplicate ("Copy of" prefix)
    - Update named position: re-captures after outside edits
    - Move up / Move down
    - Delete
  - Mate values are relative, so they can be satisfied even if both sides moved. A position references every part (unmated instances by absolute transform). Mates are not required.
  - Subassembly named positions: in the parent, "Add to named positions" adds a Position dropdown column choosing the subassembly's position. A first parent position is auto-created if none exists.
  - "Edit named position driver mate" from a mate's context menu.
  - Fastened mates are unaffected. Tangent mates may break.
  - Contexts can follow a named position (AS-50). Drawings: view context menu Explode/Position.
- **Edge cases & errors:**
  - Fails if a DOF is outside the mate limits, or after part or feature changes. The message says some mate values could not be applied.
  - `ASSEMBLY_NAMED_POSITIONS_SAVE_FAILED` / `_LOAD_FAILED` / `_NO_MATES_TO_SAVE` ("Assembly must not be empty") / `_POSITION_NOT_FOUND` / `_PARTIAL_LOAD_FAILURE` / `_LOAD_SUCCEEDED_WITH_EXTRA_MATES` / `_SAVED_MATE_NOT_FOUND_ON_LOAD` / `_NEW_INSTANCES`.
- **OCCT mapping:** Solver: apply the stored DOF values as temporary driving equalities, solve, then release them.
- **Priority:** P1

### AS-55 Display states
- **Purpose:** Named visibility snapshots (instances, subassemblies, mates, MCs).
- **Sources:** A/display_states.
- **Selection inputs:** Hide or show items normally while the dialog is open.
- **Parameters:** Display state name (text), a dropdown to apply, plus Add (+), Update, Rename (pencil), Delete.
- **Behavior / semantics:**
  - Selecting a state applies its visibility. Update redefines it from the current visibility.
  - Items added later: explicit MCs show by default in existing states. Implicit MCs and mates are hidden.
  - Drawings default to "Show all" and can pick a state per view. Changing a state flags related drawings for update.
  - Available in the View-only toolbar.
- **Edge cases & errors:** `DISPLAY_STATES_INVALID_NAME`, `DISPLAY_STATES_DUPLICATE_NAME`, `DISPLAY_STATES_UPDATE_HAD_NO_EFFECT`, `DISPLAY_STATES_APPLY_HAD_NO_EFFECT`, `DISPLAY_STATES_DOES_NOT_EXIST`.
- **OCCT mapping:** App model (a visibility map keyed by occurrence path).
- **Priority:** P2

### AS-56 Interference detection (and drag collision)
- **Purpose:** Find and show overlapping volume between instances.
- **Sources:** View__interference_detection; A/instances_list ("Check interference").
- **Selection inputs:** Two or more parts or instances (box select: left-to-right = fully enclosed, right-to-left = touching).
- **Parameters:**

| FS id | UI label | Type | Default | Notes |
|---|---|---|---|---|
| — | Parts | multi | — | ≥ 2 |
| — | Include standard content | bool | off | |
| — | Show top level only | bool | off | hides interferences internal to the selected subassemblies |
| — | Interferences | result list | — | hover cross-highlights; focusing zooms to it; the hover bbox shows L/W/H |

- **Behavior / semantics:**
  - Interference volume is shown in red. The list gives the involved part pairs. Each result's bounding box dimensions are shown.
  - Touching-only contact is not an interference (inferred).
  - **Drag collision detection** (stop or highlight on contact while dragging) is **not documented** in Onshape help. Optional noshape extension, P2.
- **Edge cases & errors:** —
- **OCCT mapping:**
  - Broad phase: `Bnd_Box` / `BVH` (OCCT `BVH_Tree`) over the instances.
  - Narrow phase, fast: `BRepExtrema_ShapeProximity` (BVH triangle overlap on meshes) or a mesh triangle-triangle test (a Rust crate such as parry3d).
  - Narrow phase, exact: `BRepAlgoAPI_Common` on transformed shapes, then `BRepGProp::VolumeProperties` > tolerance and `BRepBndLib::Add` for the bbox dims. `BRepExtrema_DistShapeShape` gives zero-distance or contact checks.
  - Drag collision: mesh BVH tests per frame, then optionally clamp the motion by bisection on the drag parameter.
- **Priority:** P1

### AS-57 Bill of materials (assembly panel)
- **Purpose:** An auto-generated, live, editable BOM of the assembly. Drawing BOM tables are covered in the Drawings section.
- **Sources:** A/bill_of_material; A/working_with_the_bom_table; A/bom_table_formatting; A/bom_table_templates.
- **Selection inputs:** n/a (rows cross-highlight instances).
- **Parameters:** BOM type **Flattened** / **Structured**. Columns: default **Item, Quantity, Part number, Description**, plus any property (Add column; Remove, Move left/right, resize; Item cannot be removed or moved). Sort by a column via a 3-way toggle (unsorted, asc, desc), one column at a time. Templates (name, type, columns, All uppercase in Drawings, Show top level assembly row).
- **Behavior / semantics:**
  - **Structured:** top-level rows. Subassemblies expand with items numbered `9.1`, `9.2`, and so on.
  - **Flattened:** all parts as if at the top level, with no subassembly rows. This is the "parts-only" view; Onshape has no separate parts-only mode.
  - Item order follows the Instances list order (reorder the list to renumber).
  - Quantity aggregates identical components (inferred grouping key: same source part, version, configuration and part number).
  - **Subassembly BOM behavior** (an assembly property): Show assembly and components (default) / Show assembly only / Show components only. It affects every BOM the assembly appears in, including drawings.
  - Editable cells write back to part properties. Released, imported or versioned sources are read-only. Standard content cannot be edited in the BOM.
  - Insert non-geometric **items** (glue, paint; quantity) from the company item list. They appear in the Instances list as items.
  - **Suppress from this BOM** (Structured only, immediate children only; shown as `--*` when "Show excluded" is on). **Exclude from all BOMs** (a part or assembly property, global, takes precedence).
  - Overflow menu: Copy table, Show/hide excluded, Expand/Collapse all, Show/hide top level assembly row (aggregated properties), Save as template, Export to CSV (only expanded rows export).
  - Right-click: Switch to Part Studio/Assembly, Open linked document, Generate next or missing part numbers.
- **Edge cases & errors:** Include/exclude is not possible in a version.
- **OCCT mapping:** App data model. Traverse the instance tree with a grouping key. No OCCT.
- **Priority:** P1

### AS-58 Assembly configurations and variable table
- **Purpose:** Configure assembly-level parameters and use variables in mate and feature values.
- **Sources:** PartStudio__part_studio_and_assembly_configurations; A/assembly_variable_table; A/exploded_views; A/instances_list (dynamic suppression).
- **Selection inputs:** Configure assembly features: click an instance, mate or pattern parameter to add a column.
- **Parameters:**
  - Configuration inputs: List, Checkbox, or Configuration variable.
  - Variable table rows: Name (a FeatureScript identifier: letter or underscore, then alphanumerics or underscore), Type, Value, Description.
- **Behavior / semantics:**
  - "In an assembly, you can only configure mates (not mate connectors), instances, and patterns": mate offsets and limits, instance suppression, instance configuration, pattern counts and distances. Exploded view step fields are configurable too.
  - Assemblies can be configured independently of Part Studio configurations.
  - Inserting a configured assembly exposes its inputs (AS-03). The configuration selector sits at the top of the Instances list.
  - Variable table: create a Variable Studio from the panel, or insert one from this or another document (versioned, read-only). Duplicate names auto-suffix (`Length_0`). The last table wins on name clashes (override icon). Renaming a variable updates its references.
  - Table menu: Copy table, Switch to Variable Studio, Open or Update linked document, Move up/down, Edit reference, Remove reference.
- **Edge cases & errors:** —
- **OCCT mapping:** App expression engine plus configuration tables (shared with Part Studios).
- **Priority:** P1

### AS-59 Standard content (fasteners)
- **Purpose:** Insert library fasteners (bolts, screws, nuts, washers, keys, PEM) with auto-size and auto-mate.
- **Sources:** A/standard_content (`Shift+Insert` = insert furthest from selection, `Alt+Insert` = closest to selection).
- **Selection inputs:** Optional preselection: hole circular edges, cylindrical faces, or planar faces containing holes. Otherwise drag-place onto an MC.
- **Parameters:** Standard (ANSI / DIN / ISO / PEM), Category, Class, Component, then component options (Size, Length, thread fine or coarse, material, etc.). Auto-size button. Part number and Description (company metadata, permission dependent). Insert / Insert closest to selection / Insert furthest from selection.
- **Behavior / semantics:**
  - **Auto-size** from a selected hole edge or cylinder. Bolts and screws round *down* (no interference). A nut sized from a bolt shaft rounds *up*.
  - Placing creates a **Fastened** mate. The `A` key flips orientation while placing.
  - A planar face preselection fastens to every hole on it (regardless of size fit).
  - Thread pitch comes from the Hole feature's fine/coarse setting, else coarse.
  - Washers and nuts have 2 attachment points, so they stack. Closest = against the bolt head side; Furthest = against the part side. A washer inserted into a hole with a bolt goes under the head.
  - Edit standard content instance: change Size, Length and similar, but not Standard, Category, Class or Component. "Select instances with the same configuration / same part and configuration" for batch edits.
  - Standard content icon in the list. Read-only in the BOM.
- **Edge cases & errors:** No matching size gives a message.
- **OCCT mapping:** Needs a custom parametric fastener generator (no OCCT library): `BRepPrimAPI_MakeCylinder`/`MakePrism` for a hex head (`BRepBuilderAPI_MakePolygon`) plus an optional cosmetic thread, from standard dimension tables (ISO 4014/4017/4032/7089, ANSI B18, DIN 6885 keys). Auto-size: `BRepAdaptor_Surface::Cylinder().Radius()`.
- **Priority:** P2

### AS-60 Assembly keyboard shortcuts
- **Purpose:** Hotkeys relevant to assemblies (customizable).
- **Sources:** Home__keyboard_shortcuts_and_hotkeys; the respective pages.
- **Selection inputs:** n/a. **Parameters:** n/a.
- **Behavior / semantics:**
  - `i` Insert parts and assemblies; `m` Fastened mate dialog; `j` show/hide mates; `h` Show mates mode; `shift+s` Snap mode; `k` show/hide MCs; `ctrl+m` MC dialog
  - `shift` lock mate inference; `a` flip primary axis (and flip a fastener while placing); `q`/`a` alignment during snap drag; `ctrl` cycle source MCs (snap/insert)
  - `shift+i` Isolate; `shift+t` Make transparent; `shift+n` rename; `y` hide selected (in-context); `s` shortcut toolbar; `alt+c` search tools
  - `shift+insert` / `alt+insert` standard content furthest/closest; rollback bar ↑/↓ for explode steps; `ctrl+c`/`ctrl+v` copy/paste instances
- **Edge cases & errors:** —
- **OCCT mapping:** UI keymap.
- **Priority:** P1

### AS-61 Assembly constraint solver (design suggestion)
- **Purpose:** (design suggestion) The noshape engine behind mates, relations, limits, drag, animate, named positions and DOF/over-definition diagnostics. **OCCT has no assembly or 3D constraint solver**, so this is custom work.
- **Sources:** Derived from the documented semantics in AS-20..AS-43; `MateDOFType` (matedoftype.gen.fs); solver-related ErrorStringEnum entries. Reference implementations: OndselSolver (LGPL, used by FreeCAD Assembly), SolveSpace (GPLv3, licence caution), PlaneGCS (LGPL, 2D only).
- **Selection inputs:** n/a.
- **Parameters:** Internal: tolerance (e.g. 1e-10 m / 1e-10 rad residual), max iterations, damping λ, drag weight.
- **Behavior / semantics:**
  - **Bodies:**
    - Each free rigid body b has a pose `T_b = (R_b, p_b)`: 6 DOF, stored as a unit quaternion plus a vector. Updates use a local 6-vector `δ = (ω, v)` with `R ← exp([ω]×) R` and `p ← p + v`, so there is no normalization constraint.
    - Fixed instances, the assembly Origin, and locked or rigid subassemblies are constants or single bodies.
    - **Pre-pass:** union-find over Fastened mates (with consistent offsets) and Groups. Each cluster becomes one body, with member transforms relative to the cluster frame. This shrinks the problem drastically.
    - Flexible subassemblies are flattened into the parent problem, one copy per occurrence.
  - **Connector frames:** `A = T_a * L_A * Off` (local MC frame L, with mate offset `Off = Trans(o) * Rot(axis, θ)`), `B* = T_b * L_B * R_align`, where `R_align = R_x(π)^(1-flip) * R_z(k·π/2)` (convention per AS-20, inferred). Write `d = o_A - o_B*`.
  - **Mate equations** (scalar residuals; each count = DOF removed):
    - Fastened (6): `d·x_B* , d·y_B* , d·z_B*`, `x_A·y_B*`, `x_A·z_B*`, `y_A·z_B*` (branch: `x_A·x_B* > 0`, `z_A·z_B* > 0`).
    - Revolute (5): `d (3)`, `z_A·x_B*`, `z_A·y_B*`.
    - Slider (5): `d·x_B*`, `d·y_B*`, `x_A·y_B*`, `x_A·z_B*`, `y_A·z_B*`.
    - Cylindrical (4): `d·x_B*`, `d·y_B*`, `z_A·x_B*`, `z_A·y_B*`.
    - Pin slot (4): `d·y_B*`, `d·z_B*`, `z_A·x_B*`, `z_A·y_B*` (B = slot MC, slot along `x_B*`).
    - Planar (3): `d·z_B*`, `z_A·x_B*`, `z_A·y_B*`.
    - Ball (3): `d (3)`.
    - Parallel (2): `z_A·x_B*`, `z_A·y_B*`.
    - Tangent (1): `g(T_a, T_b) = signed analytic distance` (AS-32), plus the side or normal branch from flip.
    - Width (3 for a single tab): `(o_T - (o_W1+o_W2)/2)·n`, `z_T·u`, `z_T·v` (AS-33).
    - Mate to Origin: B is the world frame.
  - **DOF value functions** (for limits, relations, animate, named positions, value display): with `M = B*^{-1} A`: `Tx, Ty, Tz = M.translation` components; `Rz = atan2(M.R[1,0], M.R[0,0])`, unwrapped continuously from the previous solve. Ball: Z-Y'-Z' Euler `(Rz, Ryp, Rzp)` of `M.R`, with the cone limit on `Ryp`.
  - **Limits:** inequalities `min ≤ v(q) ≤ max`. Use an active-set scheme: solve the equality system, and for each violated limit add `v = bound` as an equality and re-solve. During drag this makes the part "stop" at the limit. Alternative: a bounded LM/SQP (e.g. a projected step or a slack-variable formulation).
  - **Relations:** equality couplings between DOF value functions, using stored reference values at creation (AS-39): Gear `θ2 - θ2⁰ - s·k·(θ1 - θ1⁰)`; Rack `x - x⁰ - s·(L/2π)(θ - θ⁰)`; Screw `z - z⁰ - s·(p/2π)(θ - θ⁰)` on the same mate; Linear `x2 - x2⁰ - s·k·(x1 - x1⁰)` (default s = -1 per the transcript).
  - **Solve:**
    - Build the residual vector `F(q)` and Jacobian `J` (analytic: derivatives of dot products w.r.t. `(ω, v)` are cross-product expressions).
    - Partition the body/constraint graph into connected components and solve each independently (parallelizable).
    - Use Levenberg-Marquardt or dogleg on `min ||F||²` with minimal-norm steps (so unconstrained DOF do not drift). Warm-start from the current poses.
    - Converged if `||F||∞ < tol`.
  - **Drag:** add soft targets `w·(p_grab(q) - p_cursor)` (and the rotation target for triad rotation) with a small weight, keeping hard constraints exact (two-stage: minimize the target subject to `F = 0`, e.g. null-space projection `Δq = J⁺(-F) + (I - J⁺J) Δq_drag`). Free unmated bodies follow the target exactly.
  - **Animate / named positions:** add driving equalities `v_k(q) = value` temporarily and step with continuation (warm starts).
  - **DOF analysis:**
    - Rank of J at the solution (SVD or rank-revealing QR) gives remaining DOF per component and per body. This drives the "has DOF" triad icon and the rigid icon (0 DOF plus grounded).
    - Redundant rows with zero residual = over-defined but consistent: warn or allow, as Onshape flags `MATE_OVERDEFINED`.
    - Redundant rows with nonzero residual = **inconsistent**: red, `MATE_INCONSISTENT`.
    - Identify the culprit mate by removing constraints one at a time, or via the QR pivot order.
  - **Over-defined fallback:** solve the least-squares problem anyway and mark the mates with the largest residuals as failing (red), keeping the rest positioned.
  - **Performance:** rigid clusters and locked or rigid subassemblies collapse; incremental re-solve only of components touched by edits or drags; cache J sparsity (sparse LM, e.g. faer or nalgebra-sparse in Rust).
- **Edge cases & errors:**
  - Singular configurations: Ball Euler at Ryp = 0, parallel axes in Tangent.
  - Branch flipping (a frame flipping 180 deg): check the branch conditions after convergence and re-seed if violated.
  - Angle wrap: unwrap continuously.
  - "May not find a solution even though one exists" (help): mitigate with continuation and multiple seeds.
- **OCCT mapping:** None. Pure Rust numerics (nalgebra/faer). OCCT is used only to evaluate MC frames from topology (`BRepAdaptor_*`, `BRepGProp`) and for Tangent distance (`Extrema_ExtSS` fallback).
- **Priority:** P0

### AS-62 Assembly structure persistence and STEP/XCAF export (design suggestion)
- **Purpose:** (design suggestion) Map the assembly tree to OCCT XCAF for import/export and optional persistence.
- **Sources:** A/assembly (Export context menu); File__supported_file_formats (export formats; see the File section).
- **Selection inputs:** n/a. **Parameters:** n/a (export options belong to the File section).
- **Behavior / semantics:**
  - Each unique source (part, version, configuration) becomes one XCAF prototype shape (`XCAFDoc_ShapeTool::AddShape(shape, makeAssembly=false)`).
  - Each (sub)assembly becomes an assembly label (`NewShape()` / `AddShape(compound, makeAssembly=true)`). Each instance becomes `AddComponent(assyLabel, protoLabel, TopLoc_Location(gp_Trsf))`. Instance names go in `TDataStd_Name` on component labels, colors in `XCAFDoc_ColorTool`, materials and properties in `XCAFDoc_MaterialTool` / `TDataStd_NamedData`.
  - Export with `STEPCAFControl_Writer` (AP214/AP242, names, colors and structure kept). Use `RWGltf_CafWriter`, `RWObj_CafWriter`, and `StlAPI_Writer` (flattened) for other formats.
  - Import of STEP assemblies via `STEPCAFControl_Reader`, then walk `XCAFDoc_ShapeTool::GetComponents` to recreate the instance tree (instances fixed or grouped by default; inferred).
  - Suppressed instances are excluded. Hidden instances are exported (inferred). Mates are not representable in STEP (kinematics AP242 is optional and out of scope).
  - The native noshape persistence should be its own app format (tree + mates + features). Do not rely on XCAF for mates.
- **Edge cases & errors:** `EXPORT_ASSEMBLY_UNKNOWN_NODE_TYPE`, `EXPORT_ASSEMBLY_CREATE_INSTANCE_FAILED`, `IMPORT_ASSEMBLY_FAILED`.
- **OCCT mapping:** `TDocStd_Application`, `XCAFApp_Application`, `XCAFDoc_DocumentTool`, `XCAFDoc_ShapeTool`, `XCAFDoc_ColorTool`, `STEPCAFControl_Writer`/`Reader`, `XCAFPrs` for display styles.
- **Priority:** P1
