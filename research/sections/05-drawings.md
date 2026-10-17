## 5. Drawings

Scope: Onshape Drawing elements (2D drawings derived from Part Studios / Assemblies / parts, plus imported DWG/DXF). Drawings in Onshape are not FeatureScript features: their dialogs are app UI, so most "FS id" cells below are `-` (UI field, no FS precondition). Where FeatureScript is involved (section-view cut internals in `sectionpart.fs`, enums for GD&T / weld / tolerance / table alignment, `defineTable` for custom tables) it is cited. Help URLs are `https://cad.onshape.com/help/Content/Drawing/<page>.htm` unless stated.

Item groups: DR-01..DR-12 drawing element, creation, templates, sheets, properties framework; DR-13..DR-23 drawing properties tabs, styles, interaction basics; DR-24..DR-44 views; DR-45..DR-60 dimensions and tolerances; DR-61..DR-78 annotations, construction geometry, inspection/MBD; DR-79..DR-80 drawing sketch and constraints; DR-81..DR-88 tables; DR-89..DR-94 import/export/print/shortcuts; DR-95 drawing-engine design suggestion.

Global notes (apply to all items):
- Onshape drawings are internally DWG-database based ("All Onshape drawings are based on the .DWG file format ... DXF is also supported", drawings.htm). Sheet layers: Drawing, Title block, Border frame, Border zones.
- Simultaneous editing of one drawing tab is not supported (single active editor; others see who is editing). noshape: single-writer lock per drawing element (inferred acceptable).
- Error strings in errorstringenum.gen.fs relevant to drawings: `DRAWING_FAILED_TO_RESOLVE_VIEW_REFERENCE` "Failed to resolve view reference.", `DRAWING_PARTSTUDIO_EMPTY_AFTER_SECTION_CUT` "No part found in the part studio after section cut.", `DRAWING_ASSEMBLY_DOES_NOT_CONTAIN_VISIBLE_INSTANCES` "Assembly does not contain any visible instances.", `DRAWING_ASSEMBLY_EMPTY_AFTER_SECTION_CUT` "No instances left in assembly after section cut.", `DRAWING_VIEW_GENERATION_FAILED` "Failed to generate view.", `DRAWING_ASSEMBLY_INVALID_SECTION_CUT` / `DRAWING_PARTSTUDIO_INVALID_SECTION_CUT` "The section cut location results in invalid geometry.", `DRAWING_ASSEMBLY_NONMANIFOLD_SECTION_CUT` / `DRAWING_PARTSTUDIO_NONMANIFOLD_SECTION_CUT` "The section cut location results in non-manifold geometry.", `DRAWING_ALL_INSTANCES_HIDDEN` "All instances have been hidden.", `DRAWING_IGNORE_FAULTY_PARTS` "Faulty parts are ignored in drawing view.", `DRAWING_VIEW_CORRESPONDENCE_FAILED` "Failed to compute drawing view correspondence.", `DRAWING_EMPTY_SECTION_TARGET_BODY` "No target body for section cut.", `DRAWING_EMPTY_VIEW_CORRESPONDENCE_FAILED` "Drawing view correspondence failed. No geometry found in view region.", `EXPORT_NOT_IMPLEMENTED` "Element does not support export in selected format." Also help-documented: "Failed to resolve exploded view" (deleted explode/display state).

### DR-01 Drawing element (tab) and its context menu
- **Purpose:** A Drawing is a document tab (element) holding sheets, views, annotations and tables; it references Part Studio / Assembly geometry at a workspace, version or revision.
- **Sources:** drawings.htm, drawing_basics.htm, updating_a_drawing.htm.
- **Selection inputs:** none (tab-level).
- **Parameters:** tab context menu commands:

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Delete | command | | Last remaining tab cannot be deleted. |
| - | Open in new browser tab | command | | noshape: open in new window (inferred). |
| - | Rename | dialog | text | |
| - | Properties | dialog | metadata (Name, Part number, Revision, Description, Title 1-3, custom) | Grayed properties come from company property definitions. Edits flow into title block fields (DR-07). |
| - | Duplicate | command | | Copy tab within document; references to original Part Studio kept. |
| - | Copy to clipboard / Paste tab | command | | Pasting into another document also pastes the referenced Part Studio; no references to original document kept. |
| - | Change to version... | dialog | version/revision picker | Re-point all view references to a version (faster open; freezes drawing). |
| - | Select as document thumbnail | command | | |
| - | Update linked document... | dialog | | Update links to other documents. |
| - | Move to document | dialog | | Moves drawing and referenced Assembly/Part Studio tabs; creates links. Prevented if source doc would have no tabs. |
| - | Export | dialog | see DR-91/DR-92 | |
| - | Release / Revision history / Create task | PDM | | Out of scope (PDM) except revision data used by DR-86. |

- **Behavior / semantics:**
  - Three creation routes: from model entity (DR-02), empty drawing (DR-02), import DWG/DXF (DR-90).
  - All views of one part in a drawing are from the same version of that part; new views reuse the version used by existing views (views.htm).
  - A drawing referencing a workspace shows "needs update" whenever the referenced model changes (DR-42); referencing a version/revision avoids this and is faster to open.
  - iOS: only rename/delete/export (mobile out of scope).
- **Edge cases & errors:** concurrent edit attempt shows "who is currently editing" message.
- **OCCT mapping:** none (app data model). Drawing stored as its own element in the noshape document store with references `(elementId, version|workspace, partId/instancePath, configuration)`.
- **Priority:** P0

### DR-02 Creating a drawing (from part/curve/surface/assembly/Part Studio, or empty)
- **Purpose:** Create a new drawing tab, choose a template, optionally auto-place views.
- **Sources:** creating_a_drawing.htm, drawing_basics.htm, views.htm.
- **Selection inputs:** right-click entity in Parts list (part, curve, surface) or Instances list (subassembly) -> "Create drawing of <name>"; or "+" (Insert new tab) menu -> Create Drawing (empty; whole Part Studio/Assembly chosen later via Insert view).
- **Parameters (Create drawing dialog):**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Drawing name | text | defaults to entity name | Editable at top of dialog. |
| - | Template | list pick | see DR-03 | Required. |
| - | Custom template | button | opens DR-04 dialog | |
| - | Suggested views / No views | toggle | help inconsistent (see notes) | Bottom of dialog. |

- **Behavior / semantics:**
  - Suggested views: help pages disagree: "Four views of the part are inserted" (front/top/right/isometric per views.htm "4 standard views: top, front, right, and isometric"), while the MBD subsection says "Automates the placement of up to 6 orthographic views: front, top, left, back, bottom, right". Implement: 3 orthographic (front, top, right/left per projection angle) + isometric; with MBD data, add views whose planes carry MBD annotations (prioritising top, right, front; first-angle adapts the set).
  - No views: empty sheet; per drawing_basics.htm the drawing opens "with a view at your cursor" (Insert view preview attached to cursor; Esc cancels).
  - Template default: centermark extensions on; centermarks deletable.
  - MBD: when inserting part views, Part Studio MBD annotations (dimensions, GD&T, datums, welds, surface finishes) on the view plane appear in the view; units converted to drawing units; duplicates prevented (priority top/right/front, parents over children, insertion order); missing ones listed in MBD status panel (DR-78). Default tolerances shown on dimensions, explicit tolerances highlighted; default-tolerance changes propagate on drawing update. Dimensions on hidden lines and section views appear when those views are added. Composite parts supported.
  - Template determines starting system variables, sheet size, border entities, units, standard, projection and other properties.
- **Edge cases & errors:** drawing background color is a user preference (not per drawing).
- **OCCT mapping:** HLR for auto views (DR-27); otherwise app logic.
- **Priority:** P0

### DR-03 Template selection dialog (filters and standards)
- **Purpose:** Pick a DWT template that seeds the drawing.
- **Sources:** creating_a_drawing.htm (Selecting templates), drawing_basics.htm.
- **Selection inputs:** template list item.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Filter | list | Recently opened; Onshape; My templates (Created by me / Shared with me); Public; Teams and companies | "My templates" includes any .DWT files in documents the user can read. |
| - | Standard | tabs | All; ANSI; ISO; JIS | ANSI templates: third-angle, inch or mm. ISO: first-angle, mm. JIS: (not specified; inferred first-angle, mm). |
| - | Template (size) | list | sizes per standard | Sizes not enumerated in help; typical ANSI A-E, ISO A0-A4 (inferred). |

- **Behavior / semantics:** only templates matching selected standard and filter are shown. Onshape-owned public template sets are named like "Onshape ANSI Drawing Templates", "Onshape ISO Drawing Templates".
- **Edge cases & errors:** none documented.
- **OCCT mapping:** none; ship built-in DWT-equivalent templates (noshape native template format + DWT import).
- **Priority:** P1

### DR-04 Custom template dialog (generate a template from parameters)
- **Purpose:** Build a drawing format on the fly without a DWT.
- **Sources:** custom_drawing_templates.htm (Creating a custom template).
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Standard | enum | ANSI, ISO, JIS | Drives defaults of other fields. |
| - | Language | enum | list of UI languages | Language of title-block labels. |
| - | Size | enum | per standard | |
| - | Units | enum | Millimeters, Centimeters, Meters, Inches, Feet and inches, Foot, Yards; default by standard | Drawing units independent of workspace units; Feet and inches is drawing-only. |
| - | Decimal separator | enum | Period, Comma; default by standard | |
| - | Projection | enum | Third angle, First angle; default by standard | |
| - | Border | bool | include / do not include | |
| - | Horizontal zones | int | count | Visible when Border. Bounds undocumented. |
| - | Vertical zones | int | count | Visible when Border. |
| - | Start zones | enum | corner where zone labelling starts (bottom right / top left per sheet properties) | |
| - | Titleblock | bool | include / do not include | |
| - | Suggested views / No views | toggle | | |

- **Behavior / semantics:** generated border frame and zone labels are procedural (redrawn on sheet size/zone change); title block is static content once generated.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** none (procedural 2D geometry generator).
- **Priority:** P0 (a built-in parametric template generator is the simplest MVP path to ANSI/ISO sheets)

### DR-05 Custom drawing templates (DWT authoring, import, traditional-CAD formats)
- **Purpose:** Save a customized drawing as a reusable template; reuse company formats.
- **Sources:** custom_drawing_templates.htm, drawing_properties.htm (Update properties from a template), insert_dxf_or_dwg.htm.
- **Selection inputs:** none.
- **Parameters (Export as DWT):**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Format | enum | DWT | |
| - | Version | enum | 2013 (help); supported_file_formats lists DWT 2013, 2018 | Help says DWT export version "will be 2013". |
| - | Options | enum | Store file in a new tab / Download / both | |

- **Behavior / semantics:**
  - Template = drawing format: title block, border, zones, properties (all Drawing properties tabs), styles.
  - DWT export rules: only sheet 1 (primary template) and sheet 2 (continuation template) exported; all views deleted; only non-view geometry and text kept; content on the Drawing layer is NOT exported, only Title block, Border frame, Border zones layers.
  - Use: import DWT into any document (Import); appears under Created by me / Shared with me in DR-03.
  - Traditional CAD format workflow: create empty drawing with custom template without border/title block -> Insert DWG/DXF (DR-89) -> fix fonts -> add tables/logo (Insert image) -> add property fields via Note "Insert field" -> Move entities to Title block / Border frame / Border zones layers -> Export DWT.
  - Any sheet can swap its format to another DWT (DR-10); Drawing properties can be refreshed from a DWT (DR-12).
- **Edge cases & errors:** older DWG/DXF versions may not fully support layers.
- **OCCT mapping:** none; DWT read/write = DWG library (DR-95).
- **Priority:** P1

### DR-06 Drawing layers and "Move to"
- **Purpose:** Separate format content (title block/border) from drawing content.
- **Sources:** custom_drawing_templates.htm (Notes on Drawing layers), drawing_properties_-_formats.htm, note.htm (Moving notes), callout_balloon.htm.
- **Selection inputs:** any annotation/table/sketch entity -> context menu Move to.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | Move to | enum | Title block, Border zones, Border frame, Drawing | |

- **Behavior / semantics:**
  - Layers Border frame, Border zones, Title block each have own default color and line thickness (DR-18); entity moved there adopts layer formats (color, thickness, font).
  - When Format is Locked (DR-18) those three layers are not selectable/editable; entity must be moved back to Drawing to edit. Locked title-block fields still update when property values change.
  - DWG/DXF export puts content on layers BORDER_FRAME, BORDER_ZONES, TITLE_BLOCK.
  - Sheet size change: title block NOT replaced; border frame and zones regenerated at new size/zone count; Drawing-layer content never replaced by size change or format replacement.
- **Edge cases & errors:** none.
- **OCCT mapping:** none (layer attribute in 2D model).
- **Priority:** P1

### DR-07 Title block and automatic property fields
- **Purpose:** Title block populated from drawing/document/part metadata.
- **Sources:** sheets.htm (Editing title blocks), note.htm (Insert drawing property), drawing_properties_-_formats.htm, drawing_properties_-_annotations.htm.
- **Selection inputs:** title-block notes (right-click -> Edit note, or double-click when unlocked).
- **Parameters (Onshape-supplied template mappings):**

| FS id | Title block cell | Source property | Notes |
|---|---|---|---|
| - | Drawn, Name | Documentation name, else Nickname | Applied at creation; later changes to Documentation name do not alter existing drawings. |
| - | Drawn, Date | Created date | UTC used (may display "tomorrow"). |
| - | DWG No | Part Number (drawing property) | |
| - | Title | Description | |
| - | Rev | Revision | |
| - | Sheet number / total sheets | automatic | Updated automatically. |
| - | Size, Scale | Sheet size / Sheet scale fields | From Sheet properties. |

- **Behavior / semantics:**
  - Field entities are gray (inherit values); hovering shows property name; can be deleted and replaced with literal text or another field (Insert drawing property).
  - Empty properties render as placeholder dashes "----" with gray background; both can be hidden (Annotations > Notes: Hide property placeholder, Hide property background). Exports by default omit "----" placeholders.
  - Title block lines and labels are ordinary editable sketch lines/notes.
- **Edge cases & errors:** printed drawing shows dashes unless deleted/hidden.
- **OCCT mapping:** none (field-expression text runs, DR-62).
- **Priority:** P0

### DR-08 Sheets panel (navigation, references, pin, replace, configuration)
- **Purpose:** List sheets and their views/tables/references; manage references.
- **Sources:** sheets.htm. Shortcut Ctrl+s (toggle panel), PgDn/PgUp/Home/End.
- **Selection inputs:** sheet rows, view rows, reference rows (multi-select supported).
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | Sort by sheet / Sort by reference | toggle | | Tree: sheets->references->views, or references->sheets->views. |
| - | Create sheet | button | | Adds after highlighted sheet and displays it. |
| - | Pin view reference / Unpin | context | | Only for versioned/revisioned references; pin icon + version name; pinned refs not updated. |
| - | Switch to reference | context | | Opens source tab. |
| - | Replace | context | Insert view dialog | Replaces reference on every sheet where referenced. |
| - | Change configuration | context | Configuration dropdown | From Sort-by-sheet parent-child set: only that set; from Sort-by-reference: all views of that reference. |

- **Behavior / semantics:** current sheet highlighted with blue bar; detail/section views show identifiers; released parts show part number/state and "latest revision" icon; linked views on other sheets show arrow badge; hovering configured reference shows configuration tooltip.
- **Edge cases & errors:** none.
- **OCCT mapping:** none (UI).
- **Priority:** P1

### DR-09 Sheet operations (add, duplicate, copy/paste, delete, reorder, rename, move views)
- **Purpose:** Manage multi-sheet drawings.
- **Sources:** sheets.htm, views.htm (Moving a view to another sheet).
- **Selection inputs:** sheets / views in Sheets panel.
- **Parameters:**

| FS id | UI label | Type | Notes |
|---|---|---|---|
| - | Duplicate | context | New sheet directly after original; independent copy (e.g. change configuration per sheet). |
| - | Copy / Paste | context | Same drawing, other drawing same doc, or another workspace (source must be a version when cross-workspace). Pasted sheet goes to bottom. |
| - | Delete | context | Restorable by Undo / history restore. |
| - | Move up / Move down / drag | context | Reorder sheets. |
| - | Rename (Shift+N) | context | Renames panel entry only, not title-block sheet title. |
| - | Switch to sheet / drag view to sheet | context | Child views do not follow parent automatically; select each. Related annotations (labels, dimensions) move with view. |

- **Behavior / semantics:** templates contain main sheet + continuation sheet formats; new sheets use the continuation format (inferred from DWT sheet-2 rule). Moving an aux view does not move its parent and vice versa; a view linked to a view on another sheet keeps the link (cannot be unlinked; delete and recreate).
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P0

### DR-10 Sheet properties and replacing the sheet format
- **Purpose:** Per-sheet scale, paper size, border/zones, format.
- **Sources:** sheets.htm (Sheet properties, Replacing the drawing format), exporting_a_drawing.htm.
- **Selection inputs:** right-click empty sheet space -> Sheet properties, or Sheets panel -> Properties / double-click current sheet name.
- **Parameters:**

| FS id | UI label | Type | Values / default / bounds | Notes |
|---|---|---|---|---|
| - | Scale | ratio | N:N | Reflected in title block and as "Sheet scale" option for views. |
| - | Size | enum | template sizes + Custom | Linked to title block Size field. |
| - | Width / Height | length | Custom only; max 1800 in x 1800 in | |
| - | Border | bool | | |
| - | Horizontal zones | int | | |
| - | Vertical zones | int | | |
| - | Start zones | enum | Bottom right, Top left | |
| - | References | list | links | Parts/Part Studios/Assemblies used on sheet; link icon to open. |
| - | Select a DWT | picker | current doc / other docs / import from device / released items / version graph | Applies that DWT format to this sheet only. |

- **Behavior / semantics:** format replacement swaps border/zones/title-block layers; Drawing layer untouched (DR-06).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** none.
- **Priority:** P0

### DR-11 Reference manager (update references)
- **Purpose:** Update view references (e.g. to newer versions) in bulk or selectively.
- **Sources:** sheets.htm (Updating references), views.htm.
- **Selection inputs:** reference icon in Sheets panel.
- **Parameters:** Update all; Selective update -> choose refs -> Update selected.
- **Behavior / semantics:** updating a linked view's reference updates all linked views. Distinct from DR-42 (geometry regeneration).
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P2

### DR-12 Drawing Properties panel (framework, update from template, lock)
- **Purpose:** Drawing-wide defaults for all entities, organized in tabs.
- **Sources:** drawing_properties.htm. Access: right-click empty area -> Drawing properties, or icon on right side.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Notes |
|---|---|---|---|
| - | Tabs | | Units and Precision (DR-13), Dimensions (DR-14), Annotations (DR-15), Views (DR-16), Construction Geometry (DR-17), Formats (DR-18), Tables (DR-19), Inspection (DR-20). |
| - | Update properties from a template... | DWT picker | Copies property values from a DWT (current doc / other docs: Owned by me, company, Recently opened, Created by me, Shared with me / Import). |
| - | Lock drawing properties | bool | Prevents accidental edits. |

- **Behavior / semantics:** style resolution order (inferred from styles.htm/format_painter.htm): entity-local overrides (Styles panel or entity dialog) > Drawing properties > template defaults. Changing a Drawing property updates every entity that has no local override.
- **Edge cases & errors:** none.
- **OCCT mapping:** none (style system).
- **Priority:** P1

### DR-13 Drawing Properties - Units and Precision
- **Purpose:** Default units, precision, dual dimensions and zero suppression for all numeric text.
- **Sources:** drawing_properties_-_units_and_precision.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Primary > Units | enum | Millimeters, Millimeters (Drawing), Centimeters, Meters, Inches, Inches fractional, Feet and inches, Feet, Yards | Drawing units independent of workspace units. |
| - | Primary > Decimal separator | enum | Comma, Period | |
| - | Primary > Precision | enum | 0..6 decimals, or fractional 0..1/256 | |
| - | Primary > Tolerance precision | enum | 0..6 decimals or 0..1/256 | |
| - | Primary > Angular precision | enum | 0..6 decimals | |
| - | Primary > Angular tolerance precision | enum | 0..6 decimals | |
| - | Primary > Fit class tolerance precision | enum | 0..6 decimals | |
| - | Dual > Show dual dimensions | bool | default off | |
| - | Dual > Show dual unit | bool | | Show unit suffix on the dual value. |
| - | Dual > Dimension location | enum | Top, Bottom | Relative to primary. |
| - | Dual > Hole callout location | enum | Top, Bottom, Left, Right | |
| - | Dual > Units | enum | all units except primary | |
| - | Dual > Precision / Tolerance precision | enum | 0..6 decimals or 0..1/256 | |
| - | Length leading zeros | bool | | |
| - | Length trailing zeros | bool | | |
| - | Length tolerance leading zeros | bool | | |
| - | Length tolerance trailing zeros | bool | | |

- **Behavior / semantics:** entity precision set to "(Drawing)" follows these values live; dimension palette precision overrides allow 0..8 decimals (DR-59, discrepancy with the 6 here). Fractional units render stacked fractions rounded to nearest 1/2^n.
- **Edge cases & errors:** none.
- **OCCT mapping:** none (number formatting library).
- **Priority:** P0

### DR-14 Drawing Properties - Dimensions
- **Purpose:** Default appearance of dimensions.
- **Sources:** drawing_properties_-_dimensions.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values / default / bounds | Notes |
|---|---|---|---|---|
| - | Font | enum | font list | |
| - | Text height | length | | |
| - | Color | color | palette / hex / RGB / mixer, custom colors | |
| - | Arrowhead (length) | length | 0.004 in .. 393.7 in | |
| - | Arrowhead type | enum | None, Filled, Unfilled, Open, Oblique, Filled dot | |
| - | Text justification | enum | Left, Center, Right, Auto | Help: applies to Note and Dimension text. |
| - | Underline overridden dimensions | bool | default on | |
| - | Break dimension by default | bool | default on | Affects new dimensions only. |
| - | Text alignment | enum | Aligned with dimension line, Horizontal | |
| - | Text gap | length | | |
| - | Geometry gap | length | | |
| - | Extension past line | length | | |
| - | Lock the angle of all jogged dimensions to 45 degrees | bool | | |
| - | Chamfer > 45 degree style | enum | Note, Dimension | |
| - | Chamfer > 45 degree content | enum | Angle x Length, Length x Angle, Length x Length, C Length | |
| - | Chamfer > Non 45 degree style | enum | Dimension, Note | |
| - | Chamfer > Non 45 degree content | enum | Angle x Length, Length x Angle, Length x Length | |
| - | Chamfer > Length precision / Angle precision | enum | digits | |
| - | Ordinate dimensions style | enum | ANSI, ISO | ANSI: leader lines only; ISO: plus arrows connecting increasing values from origin. Default from template standard. |
| - | Diameter and radial > Show dimension lines by default | bool | ISO template: on; ANSI: off | New dims only. |

- **Behavior / semantics:** changes apply to all dimensions without local style overrides.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-15 Drawing Properties - Annotations
- **Purpose:** Default appearance of annotation types.
- **Sources:** drawing_properties_-_annotations.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | All annotations > Arrowhead | length | size | |
| - | Notes > Font, Text height, Color | style | | |
| - | Notes > Hide property placeholder | bool | | Hides "----". |
| - | Notes > Hide property background | bool | | Gray field background. |
| - | Flag notes > Font, Border shape, Color | style | | |
| - | Callouts > Font, Text height, Color, Text justification, Stack direction, Stack gap size | style | | |
| - | Center of mass > Diameter | length | | Size of COM indicator symbol. |
| - | Datums > Font, Text height, Datum size, Color | style | Datum size = triangle size | |
| - | Geometric tolerances > Font, Text height, Color | style | | |
| - | Surface finish symbols > Font, Text height, Color | style | | |
| - | Weld symbols > Standard | enum | ANSI, ISO (`WeldStandard`) | Independent of drawing standard. |
| - | Bend notes > Font, Text height, Color | style | | Sheet metal bend notes. |
| - | Hole callouts > Font, Text height, Color, Indicator | style | | |

- **Behavior / semantics:** as DR-14.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-16 Drawing Properties - Views
- **Purpose:** Projection angle, line styles of view geometry, view-type defaults, insert-view defaults.
- **Sources:** drawing_properties_-_views.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | All views > Projection angle | enum | First angle, Third angle | Drives projected view placement (DR-28). |
| - | All views > Visible edges | style | thickness, color | |
| - | All views > View labels | style | font, size, treatment (bold/italic), color | |
| - | Insert view defaults > View simplification | enum | None, Auto (default), Absolute (length), Ratio to studio (%), Ratio to part (%) | See DR-27. |
| - | Insert view defaults > Auto centermarks (Parts) | bool | | |
| - | Insert view defaults > Auto centerlines (Parts) | bool | | |
| - | Insert view defaults > Hidden lines | enum | Hidden, Shown | |
| - | Insert view defaults > Threads (Parts) | enum | Hidden, Shown | |
| - | Insert view defaults > Bend lines (Parts) | bool | | |
| - | Insert view defaults > Bend notes (Parts) | bool | | Visible only when Bend lines on. |
| - | Insert view defaults > Tangent edges | enum | Hidden, Solid, Phantom | |
| - | Insert view defaults > Scale label | bool | | |
| - | Hidden edges | style | thickness, color, dash spacing | |
| - | Tangent edges | style | thickness, color, dash spacing | |
| - | Exploded lines | style | thickness, color, dash spacing | |
| - | Section views > Hatch pattern | enum | type (ANSI/ISO/General) + pattern | |
| - | Section views > Hatches | style | line size, color | |
| - | Section views > Cutting lines | style | line size, color | |
| - | Section views > Arrowhead | style | size; Filled, Unfilled, Open | |
| - | Section views > Cutting line label | style | font, size, treatment, color | |
| - | Detail views > Detail profile lines | style | size, color, style Phantom/Dashed/Solid | |
| - | Detail views > Arrowhead | style | size; Filled/Unfilled/Open | |
| - | Detail views > Detail profile label | style | font, size, style, color | |
| - | Detail views > Detail profile style | enum | In line, Connected, Leader | |
| - | Detail views > Detail view profile / Show detail view profile | bool | | Outline around detail view. |
| - | Detail views > Clip border to geometry | bool | default off | |
| - | Auxiliary views > Show view reference indicator | bool | | |
| - | Auxiliary views > View reference format | enum | Line style, Arrow style | Arrow style can be dimensioned to (angular dimension). |
| - | Auxiliary views > Viewing lines | style | thickness, color | |
| - | Auxiliary views > Arrowhead | style | size; Filled/Unfilled/Open | |
| - | Auxiliary views > Viewing line label | style | | |
| - | Auxiliary views > View label prefix / suffix | text | | |
| - | Auxiliary views > View label style | enum | "A-A", "A" | |
| - | Break views > line thickness, color | style | | |
| - | Cosmetic threads | style | thread style, thickness, color (internal and external); dash length scale for ANSI | |
| - | Flat pattern > All bend lines thickness | length | | |
| - | Flat pattern > Up bend lines / Down bend lines | style | Solid, Dashed, Center (default); color | |
| - | Flat pattern > Bend tangent edges | style | thickness, color | |
| - | Flat pattern > Counterbore/Countersink up / down | style | Solid (default), Dashed, Center; color | |
| - | Flat pattern > Form tear | style | Solid (default)... | |
| - | Flat pattern > Form up / Form down | style | Center (default)... | |
| - | Flat pattern > Form centermarks | style | Solid (default)... | |

- **Behavior / semantics:** child views (section, detail, etc.) adopt the parent's insert-view defaults. Changing Projection angle re-lays out projected children (inferred).
- **Edge cases & errors:** none.
- **OCCT mapping:** HLR output classified into edge classes (visible sharp, visible smooth/tangent, hidden, silhouette) and mapped to these styles (DR-95).
- **Priority:** P0

### DR-17 Drawing Properties - Construction Geometry
- **Purpose:** Defaults for centerlines, centermarks, virtual sharps, drawing sketch geometry and Part Studio sketches shown in views.
- **Sources:** drawing_properties_-_construction_geometry.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | Created in drawing > Virtual sharps | enum + style | Centermark, Edge extension; color, thickness | |
| - | Created in drawing > Sketch geometry | style | line size, color | |
| - | Sketch driven dimensions color | color | | Color of driven dims on drawing sketches. |
| - | Part Studio sketches | style | line size, color, sketch point appearance/size | For sketches shown via Show/hide sketches. |
| - | Centermarks | style | line color, thickness, mark size, gap size, Extended lines (bool); circular: bolt circle, radial lines, pattern centermark (bools) | |
| - | Centerlines | style | line size, color, dash spacing | |

- **Behavior / semantics:** extended centermark lines have end grips; drag moves all four arms together (snaps to edges, red diamond); Shift+drag moves a single arm (no snapping).
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-18 Drawing Properties - Formats
- **Purpose:** Lock state and styling for Border frame, Border zone, Title block layers.
- **Sources:** drawing_properties_-_formats.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Locked | bool | default Locked (new drawing) | Help text contradicts itself ("should have this property initially set to unlocked"); behavior: default locked. |
| - | Border frame > Color, Thickness | style | | Inner border. |
| - | Border zone > Color, Thickness | style | | Zone labels. |
| - | Title block > Color, Thickness | style | | Lines and content. |

- **Behavior / semantics:** see DR-06/DR-07. Field values update even when locked.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-19 Drawing Properties - Tables
- **Purpose:** Default styling for each table type.
- **Sources:** drawing_properties_-_tables.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | All tables > Horizontal padding | length | default 0.06 in | |
| - | All tables > Vertical padding | length | default 0.06 in | |
| - | BOM / Custom / Cut list / Hole tables > Line thickness, Font, Header row text, Content row text | style | size, treatment, color | |
| - | Hole tables > Indicator style | enum | ANSI, ISO | Hole tag indicator. |
| - | Hole tables > Indicator arrowhead | length | | |
| - | Hole tables > Indicator lines | style | thickness, color | |
| - | Hole tables > Tags | style | font, size, bold/italic/none, color | |
| - | Revision tables > Line thickness, Font, Title row text, Header row text, Content row text | style | | |
| - | Revision tables > Revision callout | style | shape, number of characters, font, size, color, treatment | |
| - | Tables (general) > Line thickness, Font, Title row, Header row, Content row text | style | | |

- **Behavior / semantics:** cell-level formatting via Cell edit panel overrides these.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-20 Drawing Properties - Inspection
- **Purpose:** Defaults for inspection items (DR-76).
- **Sources:** drawing_properties_-_inspection.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Item parsing | enum | Parameter (default), Row | Parameter: hole callout and chamfer dims split into separate parameters; Row: consolidated. Can be mixed. |
| - | Font | enum | | |
| - | Text | style | size, treatment | |
| - | Border shape | enum | shapes | |
| - | Border size | number | | |
| - | Border color | color | | |
| - | Solid fill | bool + color | | |
| - | Automatic inspection items > Exclude callouts | bool | default off | |

- **Behavior / semantics:** see DR-76.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P2

### DR-21 Styles panel (per-entity style overrides)
- **Purpose:** Apply visual overrides to selected dimensions/annotations/hatches/sketch geometry without changing drawing-wide properties.
- **Sources:** styles.htm.
- **Selection inputs:** one or more dimensions, annotations, hatches or sketch entities; panel shows only styles common to the selection; panel unavailable if selection includes an unsupported entity.
- **Parameters:**

| FS id | UI label | Applies to | Values |
|---|---|---|---|
| - | Font, Text height, Color | dims, annotations | |
| - | Arrowhead (size + type, per end when 2 arrows) | dims (not ordinate), annotations (not datum) | None/Filled/Unfilled/Open/Oblique/Filled dot |
| - | Text justification | dims; chamfer, hole callout, callout annotations | Left/Center/Right/Auto |
| - | Text alignment | dims except ordinate | aligned / horizontal |
| - | Text gap | dims; chamfer annotation | |
| - | Geometry gap, Extension past line, Show second extension line | chamfer, max/min, 2-pt linear, point-line, line-line, line-line angular, 3-pt angular | |
| - | Show first extension line | as above + foreshorten | default on |
| - | Break dimension | all dims | default on |
| - | Show dimension line | radial, diameter | |
| - | X degree content | chamfer | Angle x Length, Length x Angle, Length x Length |
| - | Stack direction | callout | Auto, Right, Left, Up, Down |
| - | Stack gap size | callout | mm |
| - | Datum size | datum | |
| - | Hatch pattern (type, style, size), Color, Scale, Angle | hatch | |
| - | Linestyle | sketch geometry (line, circles, rectangles, spline, tangent arc) | Solid, Phantom, Center, Dashed, Dotted |
| - | Pitch scale | sketch geometry, non-solid | 0.01 .. 5.00 |
| - | Thickness, Color | sketch geometry | |
| - | Revert to drawing properties | button | clears overrides |

- **Behavior / semantics:** overrides stored per entity; revert removes them so Drawing properties apply again.
- **Edge cases & errors:** mixed unsupported selection -> panel disabled.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-22 Format painter
- **Purpose:** Copy style, tolerance/precision and/or content from one dimension/annotation to others.
- **Sources:** format_painter.htm.
- **Selection inputs:** source (1 dimension/annotation), then destinations (n).
- **Parameters:**

| FS id | UI label | Type | Default |
|---|---|---|---|
| - | Paste style | bool | |
| - | Paste tolerance and precision | bool | |
| - | Paste content | bool | |

- **Behavior / semantics:** only default / Drawing properties / Styles-panel formats are painted (not formatting applied inside the Note text editor). Cross-type painting copies only common styles.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P2

### DR-23 Drawing interaction basics (cursors, grips, snap points, inferencing, copy/paste)
- **Purpose:** Common editing behaviors shared by all drawing tools.
- **Sources:** drawing_basics.htm, drawing_dimensions.htm, note.htm.
- **Selection inputs:** n/a.
- **Parameters:** none (behavioral).
- **Behavior / semantics:**
  - Cursors: plus = pick a position; box = pick an entity.
  - Grip points: circular = move; square = resize; red = error (dangling). Types: normal resize, rotate (drag), flip (click), point attachment (drag to re-attach to point), edge attachment (drag to re-attach to edge). Dimension text has no grip (whole text draggable).
  - Snap points (orange): square = endpoint; triangle = midpoint; diamond = circle/arc quadrant; circle = arc/circle center. Midpoints and quad points disabled during dimensioning; Shift+q toggles them for current command. Callouts snap to any entity; dimension tools snap only to the entity types they accept. Dimensions do not snap to midpoints by default.
  - Inferencing: dashed red/pink horizontal/vertical alignment lines from snap points and other annotations' text midpoints during placement; hold Shift to disable. After placement no association remains from inferencing.
  - Sketch inferencing creates horizontal/vertical constraints only.
  - Copy by Alt+LMB drag for: notes, GD&T frames, datums, hole callouts, BOM tables, surface finish, weld symbols, balloons, tables. Ctrl+C/Ctrl+V of views and entities across sheets/drawings/documents (permissions apply).
  - Every non-view-geometry object has a context menu (Edit, Copy, Paste, Clear selection, Zoom to fit, Delete, plus type-specific).
  - Selection is additive; right-click Clear selection.
  - Hover highlight: dimension/annotation hover highlights referenced geometry blue; grouped annotations highlighted yellow.
- **Edge cases & errors:** none.
- **OCCT mapping:** snapping uses 2D geometry of HLR output (Geom2d curves, Geom2dAPI_ProjectPointOnCurve for nearest point); app logic otherwise.
- **Priority:** P0

### DR-24 View model (parent/child relationships, alignment, labels, creation matrix)
- **Purpose:** Data model governing how views derive from each other.
- **Sources:** views.htm, projected_view.htm, auxiliary_view.htm, drawing_section_view.htm, aligned_section_view.htm, broken_out_section_view.htm, detail_view.htm, break_view.htm, crop_view.htm, drawing_flat_pattern_view.htm.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Base view (Insert view, DR-25) has no parent. Children: projected, auxiliary, section, aligned section, detail. Modifiers applied in place to a view: broken-out section, crop, break.
  - Children inherit: scale ("Parent" by default), display style (shaded, hidden lines, tangent edges), view simplification, Show annotations from model, display state ("follow parent"); each may later be changed independently.
  - Projected/auxiliary/section children stay aligned to parent along the projection direction (drag constrained); "Suppress alignment with parent" frees placement.
  - Rotation only allowed for views with no parent alignment and no children.
  - Labels: auto-assigned letters A..Y skipping I, O, Q, S, X, Z (shared sequence for section/detail/aux, inferred). Section and detail views get labels automatically; projected/aux labels off by default (enable in View properties). Changing a child's letter also changes the letter at the parent marker (cutting line / detail circle).
  - Creation matrix (from each page's table):

| Child / modifier | Can be created from | Cannot be created from |
|---|---|---|
| Projected | base, projected, section, aligned section, auxiliary, views with broken-out sections (result also broken), flat pattern | detail |
| Auxiliary | crop views; linear edge in base, projected orthographic, auxiliary, isometric, section, aligned section views; sketch line or centerline ("In line"); flat pattern | detail, break |
| Section | auxiliary, break, crop, detail, positions in base/isometric/projected views, section and aligned section views, views with broken-out sections, flat pattern | cut line tangent to a cylindrical face |
| Aligned section | auxiliary, crop, detail, base/isometric/projected positions, section and aligned section views, views with broken-out sections, flat pattern | cut line tangent to cylindrical face |
| Detail | crop, detail, base, projected, isometric, auxiliary, section, aligned section positions, flat pattern | (none listed) |
| Broken-out section | projected, auxiliary, break, broken-out, crop, views with other broken-outs | section, detail, aligned section |
| Break | base, projected, auxiliary, section, aligned section, isometric, flat pattern | detail |
| Crop | projected, auxiliary, section, aligned section, views with broken-out sections | detail, break, flat pattern |
| Flat pattern | Insert view dialog only | any view |

  - Angular section views cannot be created on views with breaks, and breaks cannot be added to views with angular cutting lines; break locations are shared across related views.
- **Edge cases & errors:** a view linked to a view on another sheet cannot be unlinked.
- **OCCT mapping:** none (view DAG). Each view stores: source reference, view direction `gp_Ax2`, scale, 2D placement, parent id + relation params, modifiers list, display settings.
- **Priority:** P0

### DR-25 Insert view (base view)
- **Purpose:** Place a view of a part, Part Studio, assembly, surface, sketch, curve, composite part, or sheet metal flat pattern on the active sheet.
- **Sources:** views.htm (Insert view).
- **Selection inputs:** entity chosen in Select a part or assembly dialog.
- **Parameters:**

| FS id | UI label | Type | Values / default | Visible when / notes |
|---|---|---|---|---|
| - | Source location | tabs | Current workspace; Other documents (filters, version graph, released items, create version) | |
| - | Entity filter | enum | Parts (default), Part Studio (whole), Assemblies, Surfaces, Sketches, Curves, Composite parts, Sheet metal flat patterns | Filter icons appear only if source contains that type. |
| - | Show annotations from model | bool | checked | Parts only (MBD); inherited by children. |
| - | View orientation | enum | Top, Left, Right, Front (default), Back, Bottom, Isometric, Named views (with/without section planes) | Mate-connector orientation not documented for drawings. |
| - | View scale | ratio | dropdown of standard scales or N:N | Default sheet scale (inferred). |
| - | View simplification | enum | None, Absolute, Ratio to studio, Ratio to part, Auto (default) | Only affects newly inserted views. |
| - | Show sketch appearances | bool | | |
| - | Explode/Position | enum | exploded views / named positions | Assemblies only. |
| - | Display state | enum | assembly display states | Assemblies only. |

- **Behavior / semantics:**
  - Click on sheet to place (preview follows cursor). After the first view, Projected view tool auto-activates (DR-28).
  - Label and scale label off by default.
  - If a named view / explode / display state is active in the source when the Drawing tab is first created, dialog fields are pre-populated.
  - Configuration of source is selectable (via Change configuration later; DR-40).
  - If the drawing is out of date when placing a new view, it auto-updates first.
- **Edge cases & errors:** `DRAWING_FAILED_TO_RESOLVE_VIEW_REFERENCE`, `DRAWING_ASSEMBLY_DOES_NOT_CONTAIN_VISIBLE_INSTANCES`, `DRAWING_ALL_INSTANCES_HIDDEN`, `DRAWING_VIEW_GENERATION_FAILED`. Deleted named view greyed in orientation list.
- **OCCT mapping:** HLRAlgo_Projector built from gp_Ax2 of the chosen orientation (standard orthographic axes; isometric = direction (1,1,1) normalized or per projection convention); HLRBRep_Algo / HLRBRep_PolyAlgo; assemblies: TopoDS_Compound of located shapes (TopLoc_Location) per instance; sketches/curves projected directly (no HLR occlusion for wire-only content, or treat as edges in HLR).
- **Priority:** P0

### DR-26 View properties dialog
- **Purpose:** Inspect/edit a view's reference and display settings.
- **Sources:** views.htm (View properties). Open by double-click on view or context menu.
- **Selection inputs:** one or more views (multi-edit shows "multiple values").
- **Parameters:**

| FS id | UI label | Type | Values / bounds | Notes |
|---|---|---|---|---|
| - | Document / Workspace or Version / Type / Reference | read-only + link | | Reference link opens source tab. |
| - | View orientation | enum | Top, Left, Right, Front, Back, Bottom, Isometric, Named | |
| - | Scale | ratio | "N:N" or "N/N"; user input denominator forced to 1; Parent scale / Sheet scale options | Projected/aux/section default Parent. |
| - | Rotation angle | angle | 0..360 deg, flip arrow; default 0 | Only when view has no parent alignment and no children. |
| - | Tangent edges | enum | Hidden, Solid, Phantom | |
| - | View render mode | enum | Best quality, Best performance (default) | Applies to view and export. |
| - | View simplification | enum + value | None, Auto, Absolute (length), Ratio to studio (%), Ratio to part (%) | Children inherit, can override. |
| - | Display state | enum | | When available. |
| - | Sheet | enum | sheets | Moves view. |
| - | Name | text | default "<view perspective>-<part name>" | Shift+N rename. |
| - | View label | bool + text | | Detail/Section: plus custom prefix and suffix (multi-line label). |
| - | Scale label | bool | | |
| - | Show sketch appearances | bool | | |
| - | Show annotations from model | bool | | |
| - | Move to sheet | dialog | | |
| - | BOM table reference | enum | assemblies | |
| - | Cut list table reference | enum | cut lists | |
| - | Apply parent view section cut | bool | | Projected views created from section views. |

- **Behavior / semantics:**
  - Rotation is about the view rectangle center (detail: about detail circle center; geometry and circle size unchanged). Rotation angle != 0 disables alignment-reconnect commands. Dimensions follow rotation: horizontal/vertical stay horizontal/vertical; aligned stay aligned to geometry. Scale/label recentered below view.
  - Copy view label with Ctrl+C (takes Note default properties) or Alt+drag (keeps label properties).
- **Edge cases & errors:** views with children cannot rotate even if alignment suppressed.
- **OCCT mapping:** rotation = gp_Trsf2d applied to view 2D output; scale = 2D scale.
- **Priority:** P0

### DR-27 View display modes: hidden lines, tangent edges, shaded, render mode, simplification
- **Purpose:** Core view rendering semantics (hidden line removal and edge classification).
- **Sources:** views.htm, view_context_menu.htm, drawing_properties_-_views.htm.
- **Selection inputs:** view(s).
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Show/Hide hidden lines | toggle | hidden (default) | Hidden edges drawn dashed with Hidden edges style; dimensionable once shown. |
| - | Tangent edges | enum | Solid (default per context menu page), Phantom, Hidden | Insert-view default configurable. |
| - | Show/Hide shaded view | toggle | not shaded (default) | Decals only visible when shaded; detail inherits parent shading but can be changed. |
| - | View render mode | enum | Best performance (default), Best quality | Use Best quality if edges render incorrectly. |
| - | View simplification | enum | None / Auto / Absolute / Ratio to studio / Ratio to part | Features smaller than threshold hidden; whole parts smaller than threshold vanish (Absolute, Ratio to studio); Ratio to part keeps all parts. |

- **Behavior / semantics:**
  - Visible edges = sharp edges + silhouette (outline) edges of curved faces visible from the view direction; tangent (G1-smooth) edges classified separately for the Tangent edges setting.
  - "Best performance" = mesh-based HLR (approximate); "Best quality" = exact B-rep HLR (inferred mapping).
  - Faulty parts: shown by default in part drawings, hidden by default in assembly drawings (`DRAWING_IGNORE_FAULTY_PARTS`).
  - Part intersections (virtual edges where parts intersect) hidden by default; showing them also restores parts wrongly omitted due to interference (assemblies > 20 parts note).
- **Edge cases & errors:** misclassified hidden/visible in interfering assemblies -> Show part intersections.
- **OCCT mapping:** HLRBRep_Algo (exact) + HLRBRep_HLRToShape (VCompound visible sharp, Rg1LineVCompound tangent/smooth, RgNLineVCompound seam-like, OutLineVCompound silhouettes, and H* hidden counterparts); HLRBRep_PolyAlgo + HLRBRep_PolyHLRToShape over BRepMesh_IncrementalMesh for performance mode. Shaded: offscreen raster (OCCT V3d or noshape renderer) embedded as image in view. Simplification: Bnd_Box size test per face/feature/body; drop faces via BRepAlgoAPI_Defeaturing or drop small bodies. Part intersections: BRepAlgoAPI_Section between overlapping instance pairs (BRepExtrema/ Bnd_Box prefilter).
- **Priority:** P0

### DR-28 Projected view
- **Purpose:** Fold out an orthographic (or isometric, diagonal drag) view from an existing view.
- **Sources:** projected_view.htm. Shortcut: p.
- **Selection inputs:** parent view (1).
- **Parameters:** none in dialog; placement by drag. Shift+click places the view "with cuts from the parent view" (= Apply parent view section cut).
- **Behavior / semantics:**
  - Drag direction from parent determines projection (left/right/up/down orthographic; diagonal drags produce isometric-type views (inferred from "drag in different directions to see possible projected views")). Placement side follows projection angle: third angle places the view on the side it is viewed from; first angle opposite.
  - Tool stays active until toggled off; auto-active after first view placed.
  - Label and scale off by default; scale = Parent.
  - Projected view retains parent settings; from broken-out views, result is also broken.
- **Edge cases & errors:** not from detail views.
- **OCCT mapping:** new gp_Ax2 = parent axes rotated 90 deg about parent X or Y; HLR as DR-27; section-cut inheritance = reuse parent's cut solid.
- **Priority:** P0

### DR-29 Auxiliary view
- **Purpose:** Orthographic view folded 90 deg from a slanted edge (or about a sketch line/centerline, or down a hole axis).
- **Sources:** auxiliary_view.htm, drawing_properties_-_views.htm (Auxiliary views).
- **Selection inputs:** one linear edge in parent view, or sketch line / centerline (In line), or hole (Axis).
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | View label | text | auto A..Y skipping I,O,Q,S,X,Z | |
| - | Type | enum | Normal (perpendicular to linear edge), In line (about sketch line or centerline), Axis (down axis of a hole) | |
| - | Edge / line | selection | 1 | |
| - | Reverse | toggle | | Flip view direction. |
| - | Show view reference indicator | bool | checked | |

- **Behavior / semantics:** view direction = perpendicular to selected edge within parent view plane (Normal) i.e. looking along the in-plane normal of the edge; placed along that direction, aligned. Scale = Parent. Selecting the view highlights the source edge. Reference indicator: drag elbow snap to lengthen/shorten; middle snap moves it between edge and view. Indicator context menu: Edit auxiliary view, Flip direction, Clear selection, Zoom to fit, Delete. Moving aux view does not move parent and vice versa.
- **Edge cases & errors:** not from detail or break views.
- **OCCT mapping:** compute 3D direction from the 2D edge via inverse of parent projector (edge direction in parent's gp_Ax2 plane -> 3D); new gp_Ax2 with main direction = parent-plane vector perpendicular to the edge; HLR.
- **Priority:** P1

### DR-30 Section view (full, jogged/offset, partial, angular)
- **Purpose:** Cut a view with a cutting-plane line (possibly jogged) and display the cut geometry with hatching.
- **Sources:** drawing_section_view.htm; FS internals `jogSectionPart` / `jogSectionPartInternal` / `sectionTransformedParts` (sectionpart.fs).
- **Selection inputs:** parent view; 2+ snap points defining the cutting line (click positions; snapping to geometry points makes the line associative).
- **Parameters (dialog):**

| FS id | UI label | Type | Values / default | Visible when / notes |
|---|---|---|---|---|
| - | Direction | enum | Vertical, Horizontal, Angular | |
| - | View label | text | auto A..Y skipping I,O,Q,S,X,Z | Label on by default, scale label off. |
| - | Show cut geometry only | bool | off | Show only section faces (no background geometry). |
| - | Cut angle | angle | numeric; Opposite direction flip | Angular only. Second snap point also sets angle. |
| - | Section depth | enum | Up to entity (pick entity), Blind (depth value) | Limits how far behind the cut geometry is shown. |
| - | Exclude from cut | bool + parts selection | | Assembly views only. |
| - | Cut from section | bool | off for section-from-section (on by default for aligned) | When parent is a section view: cut the already-sectioned geometry vs base geometry. |
| (FS) `jogPoints` | cutting line points | array of 3D points | 2 for straight; up to 4 total for angular jogged | |
| (FS) `isPartialSection` | partial | bool | | Set when section line shortened inside part. |
| (FS) `offsetPoints` / `offsetDistances` | offset | array | | Offset section (2-point line with offset). |
| (FS) `excludedPartsQuery` | excluded parts | Query | | |
| (FS) `keepSketches` | | bool | | |

- **Behavior / semantics:**
  - Click points to place the line (hover snap points); extra clicks add jog points (jogged/offset section). Dragging the preview to one side picks the viewing side (flip); after placement, context menu Flip direction swaps arrows/labels and regenerates.
  - Cutting line ends have elbow snap points: drag to shorten/lengthen; shortening inside the part -> partial section (only the region swept by the shortened line is cut).
  - Edit section (context menu) reopens dialog; add jog segments by dragging snap points to new geometry points; remove a segment by clicking its snap point (orange box). Moving a line placed on a snap point re-associates to new point.
  - Section view placed aligned to parent perpendicular to the cutting line; scale = Parent; selecting the view highlights the cut line in parent.
  - Hatch is applied automatically to cut faces; hatching does not overlap annotations; edit via Edit hatch (DR-37) or Drawing properties.
  - Section of a section: Cut from section option. Section from broken-out view: new cut uses base geometry, not broken-out geometry. Horizontal/vertical sections and break views can coexist (break locations shared).
  - Algorithm (sectionpart.fs `jogSectionCut`): delete non-target bodies; compute target bbox in sketch-plane CS (expanded 1%); project jog points to plane; validate jog direction monotonic (`checkJogDirection`, not for aligned); build polygon (`jogPolygon`) covering the half-space "in front of" the polyline (positive side removed) clipped to bbox, limited for partial sections; extrude the polygon through the bbox depth and subtract ("sketchAndExtrudeCut"); keep composites containing targets; tag bodies with attributes for metadata tracking; on error delete everything (empty view). Section depth/"Blind" limits the kept region behind the plane (inferred: additional cut at depth).
  - Show/hide offset cut lines (context menu on section view) toggles display of the jog transition lines.
- **Edge cases & errors:** cut line tangent to cylindrical face not allowed; `DRAWING_PARTSTUDIO_EMPTY_AFTER_SECTION_CUT`, `DRAWING_ASSEMBLY_EMPTY_AFTER_SECTION_CUT`, `DRAWING_*_INVALID_SECTION_CUT`, `DRAWING_*_NONMANIFOLD_SECTION_CUT`, `DRAWING_EMPTY_SECTION_TARGET_BODY`. Angular sections not allowed on views with breaks. Half section: not documented as a distinct mode; achievable as partial section with shortened line (inferred).
- **OCCT mapping:** cutting tool = BRepPrimAPI_MakePrism of a planar face built from the jog polygon (BRepBuilderAPI_MakePolygon + BRepBuilderAPI_MakeFace) extruded along sketch-plane normal through bbox; BRepAlgoAPI_Cut(target, tool) (for assemblies, per located instance); section faces identified via BRepAlgoAPI history (`Generated`/`Modified` of tool faces) or by classifying result faces lying on tool faces; hatch section faces (Geom2dHatch_Hatcher, DR-37); HLR of the result from the section view direction (DR-27). Simple straight section could use BRepAlgoAPI_Section for face outlines + half-space cut (BRepPrimAPI_MakeHalfSpace + BRepAlgoAPI_Cut).
- **Priority:** P0

### DR-31 Aligned section view
- **Purpose:** Section where a portion cut on a non-parallel plane is revolved into the viewing plane (cylindrical, partially symmetric parts).
- **Sources:** aligned_section_view.htm; FS `jogSectionPartInternal` `isAlignedSection`, `alignedSectionRotateAndCut` (sectionpart.fs).
- **Selection inputs:** three points: Center, Viewing plane location, Revolved plane location.
- **Parameters:**

| FS id | UI label | Type | Notes |
|---|---|---|---|
| - | View label | text | |
| - | Center | point pick | Center of section cutting line (rotation axis). |
| - | Viewing plane location | point pick | Point defining the segment kept in the view plane. |
| - | Revolved plane location | point pick | Point defining the segment revolved into alignment. |
| - | Show cut geometry only | bool | |
| - | Exclude from cut | bool + parts | Assembly only. Note: FS `jogSectionPartInternal` requires `excludedPartsQuery == undefined` when aligned and code comments "Aligned Section does not support excluded parts" - help and FS disagree; treat as unsupported or newer feature. |
| - | Cut from section | bool | Default on when parent is section/aligned section. |

- **Behavior / semantics:** dragging to place flips viewing-plane arrows depending on side. Edit via context menu Edit; dangling entities shown red in dialog. Algorithm (`alignedSectionRotateAndCut`): jog points = [viewing-plane point, Center, revolved-plane point] projected to the sketch plane; rotation axis passes through Center (jogPoints[1]) with direction = cross(startLine, endLine) (sketch-plane normal if the segments are collinear); rotation angle = 180 deg - angle between the two segments; a rotated copy of the parts is made so the revolved segment's cut lies in the viewing plane, both pieces are cut, and section faces are tracked on original and rotated bodies for hatching/metadata.
- **Edge cases & errors:** cut line tangent to cylindrical face not allowed.
- **OCCT mapping:** split target with two half-space/prism tools along each segment (BRepAlgoAPI_Cut / BRepAlgoAPI_Splitter), rotate revolved piece with BRepBuilderAPI_Transform (gp_Trsf::SetRotation about axis through Center), fuse/compound with in-plane piece, HLR.
- **Priority:** P2

### DR-32 Broken-out section view
- **Purpose:** Cut away a region (closed spline profile) of an existing view to a depth to reveal interior.
- **Sources:** broken_out_section_view.htm; FS `jogSectionPart` (`isBrokenOut`, `brokenOutPointNumbers`, `brokenOutEndConditions`, `offsetPoints`).
- **Selection inputs:** closed spline drawn on the view (Spline tool, Spline point tool to add points); depth entity (point/edge in another view) for Up to entity.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | Profile | spline points | closed | Auto-closed if open (FS adds closing point). |
| - | End type | enum | Blind (depth), Up to entity (point or edge on another view) | Depth point associative to model. |
| - | Offset | bool + length | | Offsets the cut depth. |
| - | Exclude from cut | bool + parts | | Assembly only. |

- **Behavior / semantics:** multiple broken-outs per view supported (FS stores per-broken-out spline points followed by depth point: `jogPoints = [pts1, depth1, pts2, depth2, ...]`, `brokenOutPointNumbers` sizes). Each: spline extruded from the depth point toward the viewer through bbox and subtracted. Context menu: Edit section; Remove section (keeps spline for reference; delete it separately). Hatch auto-applied. Model change moving the depth point updates spline/depth; failures show spline red -> redefine depth.
- **Edge cases & errors:** not on section, detail, aligned section views.
- **OCCT mapping:** Geom_BSplineCurve via GeomAPI_Interpolate (periodic) on view plane mapped to 3D; BRepBuilderAPI_MakeFace; BRepPrimAPI_MakePrism from depth plane toward viewer; BRepAlgoAPI_Cut; HLR.
- **Priority:** P1

### DR-33 Detail view
- **Purpose:** Enlarged view of a region of an existing view.
- **Sources:** detail_view.htm, drawing_properties_-_views.htm (Detail views).
- **Selection inputs:** parent view; region drawn with circle (center + radius click), rectangle, spline or polygon (click points, close), or existing closed region (Esc, then click region).
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Region tool | enum | Circle, Rectangle, Spline, Polygon | |
| - | View scale | ratio | | |
| - | View label | text | auto letter | |
| - | Profile style | enum | In line (label at source), Connected (dashed connector line, no label), Leader (label with line and arrow) | |
| - | Show detail view profile | bool | on | Outline around detail view matching source outline. |
| - | Clip border to geometry | bool | off | Show only geometry without border. |

- **Behavior / semantics:** detail is a 2D zoom of the parent view region (geometry clipped to profile, scaled); inherits parent shading/display state ("follow parent" only). Resize via grip between arrows; move via center grip; label leader via context Add leader. Selecting highlights circle in parent. Diameter/radial dims on detail views become foreshortened (DR-54). Edit via context menu.
- **Edge cases & errors:** no restrictions listed.
- **OCCT mapping:** no 3D op; 2D clip of parent HLR result: intersect Geom2d curves with profile (Geom2dAPI_InterCurveCurve) and trim; scale with gp_Trsf2d. Shaded detail: crop raster.
- **Priority:** P0

### DR-34 Break view
- **Purpose:** Shorten a long view by removing a strip between two break lines.
- **Sources:** break_view.htm, drawing_properties_-_views.htm (Break views).
- **Selection inputs:** view; two click positions for break lines.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | Orientation | enum | Horizontal, Vertical | |
| - | Break line type | enum | Zig zag, Small zig zag, Curve, Straight | |
| - | Gap | length | gap distance between break lines after break | |

- **Behavior / semantics:** geometry between the two lines removed and the two halves closed up to the gap; dimensions spanning the break show true (unbroken) values (standard practice, inferred). Delete a break by selecting it + Delete. Shared break locations across related section views.
- **Edge cases & errors:** not on detail views; not with angular sections.
- **OCCT mapping:** 2D post-process: clip HLR curves against strip (Geom2d trimming), translate far side by (strip width - gap); draw break-line symbols. No 3D op.
- **Priority:** P1

### DR-35 Crop view
- **Purpose:** Keep only geometry inside a closed boundary.
- **Sources:** crop_view.htm; FS `isCropView` in jogSectionPart.
- **Selection inputs:** view; boundary drawn as Circle (center + edge), Spline (click points, double-click closes; Add spline point), or Polygon (click corners, double-click closes).
- **Parameters:** shape type; boundary points.
- **Behavior / semantics:** implemented in FS as an extrude cut through the full bbox depth of the complement of the boundary (no depth). Context menu Edit crop / Remove crop (boundary remains for reference).
- **Edge cases & errors:** not on detail, break, flat pattern views.
- **OCCT mapping:** either 3D (BRepAlgoAPI_Common with prism of boundary face through bbox, then HLR - matches Onshape, keeps silhouettes correct) or 2D clip of HLR output.
- **Priority:** P1

### DR-36 Flat pattern view (sheet metal)
- **Purpose:** Show sheet metal flat pattern with bend lines, bend notes, form outlines.
- **Sources:** drawing_flat_pattern_view.htm, drawing_properties_-_views.htm (Flat pattern views), view_context_menu.htm.
- **Selection inputs:** Insert view dialog, filter Sheet metal flat patterns.
- **Parameters:** as DR-25; display: bend lines (on by default, toggle), bend notes (toggle), styles from DR-16.
- **Behavior / semantics:** shows bend lines (up/down styles), form outline where cut, form outlines on top/bottom faces, form centermarks, large diameter of counterbore/countersink holes on top and bottom faces. Bend notes come from Part Studio bend specs. Hole callouts can be applied to flat patterns. Show/hide sketches useful to show flat pattern sketches.
- **Edge cases & errors:** cannot be created from other views.
- **OCCT mapping:** flat pattern geometry from sheet metal module (section on sheet metal); 2D: planar face outline projection (BRepAlgo / direct edge projection, no HLR needed); bend lines as attributed edges.
- **Priority:** P1

### DR-37 Hatching: section hatch editing and Hatch region tool
- **Purpose:** Hatch patterns on section faces and on user-defined closed regions.
- **Sources:** drawing_sketch_tools.htm (Hatch region), view_context_menu.htm (Edit hatch), styles.htm (Hatch styles), drawing_properties_-_views.htm.
- **Selection inputs:** closed sketch profile in drawing (or drawn in dialog with sketch tools), or existing section hatch.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Standard | enum | ANSI, ISO, General | |
| - | Pattern | enum | material patterns per standard | Pattern list not enumerated in help (e.g. ANSI31 iron etc., inferred). |
| - | Scale | enum | Auto, 1.0 .. 10.0 integer steps | |
| - | Angle | enum | Auto, 0 .. 180 deg in 15 deg steps | |
| - | Color | bool + color | "Drawing properties" checked = global color | Custom color visible after accept. |

- **Behavior / semantics:** hatch region allowed in projected, auxiliary, section*, aligned section*, broken-out*, crop views (* auto-hatched on creation); not in detail or break views (help table). Edit hatch via context menu; delete via Delete. Spinner during creation; Esc cancels. Highlight colors orange/yellow/light blue as described. Group/Remove view from group can combine a hatch with a view. Section hatches in assemblies typically vary angle per part (inferred, not documented).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** Geom2dHatch_Hatcher + Geom2dHatch_Intersector (HatchGen) to clip pattern lines to face/region boundaries in 2D; pattern definitions as line families (AutoCAD .pat format compatible, inferred).
- **Priority:** P0 (section hatch) / P1 (hatch region tool)

### DR-38 View context menu: Show/hide options
- **Purpose:** Per-view visibility toggles.
- **Sources:** view_context_menu.htm.
- **Selection inputs:** view (right-click).
- **Parameters:**

| FS id | Toggle | Default | Notes |
|---|---|---|---|
| - | Hidden lines | hidden | |
| - | Bend lines | shown (flat pattern) | |
| - | Bend notes | | Flattened sheet metal views. |
| - | Shaded view | off | |
| - | Threads | shown | Cosmetic thread lines (DR-44). |
| - | Faulty parts | shown in part drawings, hidden in assemblies | |
| - | Part intersections | hidden | |
| - | Offset cut lines | | Section views. |
| - | Show/hide sketches (Part Studio) | none | Dialog: multi-select sketches from the Part Studio; works in section views too. |
| - | Show/hide sketches (Assembly) | hidden | All sketches inserted in assembly toggled together. |
| - | Sketch points | shown | |
| - | Show/hide edges | | Dialog: click edges to mark hidden (thick gray); hidden lines shown gray while picking; accept. |
| - | Centerlines | shown (per view_context_menu.htm) | Automatic centerlines on holes/cylinders/spheres; manual centerlines unaffected. drawing_construction_tools.htm says they are hidden by default (doc conflict); Insert view default "Auto centerlines" decides. |
| - | Auto centermarks | shown when view normal to hole face | Patterned/mirrored holes also; circular/linear patterns get connecting lines. |
| - | Show/hide parts | all shown | Dialog with Hidden parts selection. |
| - | Constraints | hidden | Drawing sketch constraints. |
| - | Center of mass | hidden | Parts/composites with material; dimensionable. |

- **Behavior / semantics:** options shown only when applicable (e.g. centerlines only with circular geometry).
- **Edge cases & errors:** hiding all instances -> `DRAWING_ALL_INSTANCES_HIDDEN`.
- **OCCT mapping:** filters on HLR input (hidden parts removed from compound) or output (edge ids hidden); center of mass via BRepGProp::VolumeProperties.
- **Priority:** P1

### DR-39 View context menu: orientation, tangent edges, line style, order, align/rotate, suppress alignment
- **Purpose:** View formatting commands.
- **Sources:** view_context_menu.htm.
- **Selection inputs:** view; for align/rotate a second view or a straight edge.
- **Parameters:**

| FS id | Command | Values | Notes |
|---|---|---|---|
| - | View orientation | Top, Left, Right, Front, Back, Bottom, Isometric | Current greyed. |
| - | Tangent edges | Hidden, Solid (default), Phantom | |
| - | Adjust linestyle | Edges (selection), Linestyle, Dashes, Thickness, Color | Per-edge override; persists by edge id. |
| - | Order | Bring to front, Send to back | Layer stacking. |
| - | Align views | Vertically, Horizontally | Align view centers with a picked view. |
| - | Rotate view vertically / horizontally | pick straight edge | Rotates view so edge is vertical/horizontal (rotatable views only). |
| - | Suppress alignment with parent | toggle | Breaks child-to-parent alignment only. |

- **Behavior / semantics:** see DR-26 rotation rules.
- **Edge cases & errors:** rotate disabled for views with children/alignment.
- **OCCT mapping:** none (2D transforms).
- **Priority:** P1

### DR-40 View context menu: display state, explode/position, configuration
- **Purpose:** Assembly representation per view.
- **Sources:** view_context_menu.htm, sheets.htm (Change configuration), cut_list_table.htm.
- **Selection inputs:** view.
- **Parameters:**

| FS id | Command | Values | Notes |
|---|---|---|---|
| - | Display state | list of assembly display states | Parent default "show all"; children "follow parent"; section/detail always follow parent. |
| - | Explode/Position | exploded views / named positions | Drawing shows all explode steps (rollback in explode tree ignored). Exploded lines styled per DR-16. |
| - | Change configuration | configuration picker | Per view set or per reference (DR-08). |

- **Behavior / semantics:** new display states/explodes created after drawing creation require drawing update. Deleting one -> "Failed to resolve exploded view" on update; views become empty but remain.
- **Edge cases & errors:** as above.
- **OCCT mapping:** assembly evaluation produces the instance transform set; HLR on that.
- **Priority:** P1

### DR-41 Moving, copying, deleting, grouping views; replicate annotations
- **Purpose:** View navigation and annotation reuse.
- **Sources:** views.htm (Navigating views), view_context_menu.htm (navigation, Select for replication).
- **Selection inputs:** view(s); region rectangle for region replication.
- **Parameters:**

| FS id | Command | Notes |
|---|---|---|
| - | Delete | Delete key / context. |
| - | Move | drag (children constrained by alignment). |
| - | Move to sheet | DR-09. |
| - | Copy / Paste (Ctrl+C/V), Alt+drag | Same sheet, other sheet, other drawing in same workspace; not across workspaces/documents. |
| - | Paste as table | from Excel/Sheets clipboard (DR-82). |
| - | Group / Remove view from group | Combine hatch/notes with view so they move together. |
| - | Switch to... | Opens source Part Studio/Assembly. |
| - | Select for replication > View / Region | Copies view annotations (all, or within adjustable rectangle) to clipboard. |
| - | Replicate annotations | Pastes onto another view with like geometry (views need not be linked); region mode places a box to choose target area; notification reports count replicated. |

- **Behavior / semantics:** replication re-binds each annotation's references to matching geometry in target view (matching by geometric correspondence, inferred). Hole callout replicate places copies near original; cannot replicate from source view onto itself.
- **Edge cases & errors:** unmatched annotations not replicated (count shown).
- **OCCT mapping:** correspondence by comparing 2D edge geometry/topology after normalizing transform; app logic.
- **Priority:** P1

### DR-42 Updating a drawing
- **Purpose:** Regenerate views when referenced models change.
- **Sources:** updating_a_drawing.htm. Shortcut Ctrl+q ("Updates all views and properties on each sheet").
- **Selection inputs:** none.
- **Parameters:** Update button (active when out of date, grey otherwise).
- **Behavior / semantics:** triggered by any change in referenced Part Studio/Assembly (even trivial, e.g. hiding a plane). Updates views only (not links to other documents). Associated annotations re-resolve; unresolved become dangling (DR-43). Drawing auto-updates when a new view is inserted while out of date. Versioned references never go out of date (use Change to version / Reference manager to move forward). Callout item numbers re-sync with BOM order on update.
- **Edge cases & errors:** view generation errors (see global list).
- **OCCT mapping:** re-evaluate models, rerun cuts + HLR per view (cache per view keyed by model state hash), remap annotation references by persistent ids (DR-95).
- **Priority:** P0

### DR-43 Dangling entities and reattachment
- **Purpose:** Handle annotations that lost references after update.
- **Sources:** dangling_entities.htm, drawing_dimensions.htm (Troubleshooting), drawing_-_ordinate_dimension.htm.
- **Selection inputs:** red grip point of the dangling entity.
- **Parameters:** none.
- **Behavior / semantics:** dimensions, section lines, note leaders, callouts, centermarks, GD&T can dangle; shown red with red grip at the lost reference; drag grip onto a new reference to repair; other references preserved. Ordinate dimension whose feature disappears stays red and can be deleted. Broken-out depth failure shows spline red.
- **Edge cases & errors:** none further.
- **OCCT mapping:** none (reference resolution layer).
- **Priority:** P0

### DR-44 Cosmetic threads in drawing views
- **Purpose:** Standard thread representation for tapped holes and external threads.
- **Sources:** hole_thread_callout.htm (Hole/Thread drawing lines), drawing_properties_-_views.htm (Cosmetic threads), view_context_menu.htm (Show/hide threads).
- **Selection inputs:** none (automatic from Hole / External thread features).
- **Parameters:** style per DR-16 (thread style, thickness, color internal/external; ANSI dash length scale); per-view show/hide.
- **Behavior / semantics:** ANSI: dashed lines for tapped/tapered pipe tap holes; ISO: 3/4 circle (arc) outline in end view; occluded thread portions hidden; 45 deg tapered run-out transition at thread end; external threads with chamfer/undercut show dashed thread lines plus undercut geometry. Threads are dimensionable (2-point, point-line, line-line on thread lines; smart dimension "thread" entity).
- **Edge cases & errors:** none.
- **OCCT mapping:** synthesize thread curves from hole/thread feature attributes (minor/major diameter, depth, axis) as additional 3D wires (circles/lines) fed to HLR so occlusion is computed (HLRBRep handles edges of shapes; add as a separate non-occluding edge set plus occlusion test via HLR with the solid); ISO 3/4 arc generated in 2D for end views.
- **Priority:** P1

### DR-45 Drawing dimension framework (associativity, placement, override, jog, break, flip)
- **Purpose:** Common behavior of all drawing dimensions (driven/reference dimensions measured from view geometry).
- **Sources:** drawing_dimensions.htm, drawing_basics.htm.
- **Selection inputs:** per tool (DR-46..DR-58); entity types: view edges (lines, arcs, circles, silhouettes, splines for some), vertices/snap points, centermarks, centerlines, sketch entities, thread lines, COM indicator, virtual sharps, auxiliary view arrow.
- **Parameters:** dimension palette (DR-59), styles (DR-21).
- **Behavior / semantics:**
  - All drawing dimensions are associative driven dimensions: value = measured model value at view scale-independent true size (projected length for orthographic views; true size requires edge parallel to view plane; isometric dims measure projected 2D length (inferred)). They never drive the model. Drawing sketch dimensions are a separate case (driven color in DR-17).
  - Placement: after picking references, dimension follows cursor in preview; text drag direction selects measurement mode: drag up/down -> horizontal; drag sideways -> vertical; drag perpendicular to the line through the two points -> aligned (parallel). Text may go outside extension lines. Horizontal/vertical projected snaps to other text/dims while placing.
  - Hover over circular edge "wakes up" its center mark (stays visible) for use as reference.
  - Editing: drag any grip to another edge/point/arc/center (re-association, other references kept); dimension center snap can be dragged to quadrant; text dragged freely.
  - Jogged extension lines: right-click extension line -> Add jog (jog at midpoint, then drag snap points). Lock jog angle to 45 deg via DR-14.
  - Break dimension / Unbreak dimension (context): gap where dimension lines cross; only one of two intersecting lines broken.
  - Flip arrows: click node near arrow/tick; or drag value to other side.
  - Overridden dimension: editing the value text converts to non-associative override; text always underlined (toggle in DR-14; exports can hide underline). Clearing the value field restores associativity. Underlined value = not to scale (drafting convention).
  - Dimensioning hidden lines allowed once shown.
  - Context menu: Edit (palette), Paste, Clear selection, Zoom to fit, Delete. Deleting a dimension deletes callouts grouped to it.
- **Edge cases & errors:** dangling (red) per DR-43.
- **OCCT mapping:** measure on 3D geometry of referenced entities projected to view plane (gp_Pnt -> HLRAlgo_Projector::Project) or directly in 2D on HLR curves (Geom2d), using BRepExtrema_DistShapeShape / GeomAPI_ExtremaCurveCurve for distances; BRepGProp for arc length.
- **Priority:** P0

### DR-46 Smart Dimension tool
- **Purpose:** One tool that infers dimension type from selected entities.
- **Sources:** drawing_-_dimension.htm. Shortcut: d.
- **Selection inputs:** 1-2 entities: circle, arc, circle center, line, centermark, point, thread.
- **Parameters:** none (palette after placement).
- **Behavior / semantics:**
  - Inference (inferred mapping consistent with sketch Dimension tool): point+point -> linear; point+line -> point-to-line; line+line parallel -> line-to-line distance; line+line non-parallel -> angle; single line -> length; circle/arc -> diameter/radius; two concentric circles/arcs -> wall thickness (radial distance between them).
  - Linear dimension between two silhouette edges of a cylinder in a projected (side) view: selecting the two edges (not corner points) auto-adds diameter symbol (projected views only; Line-to-line tool does this in projected and section views).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** as DR-45; concentric test via gp_Circ centers coincident.
- **Priority:** P0

### DR-47 2-point linear dimension
- **Purpose:** Distance between two points; horizontal, vertical or aligned ("rotated").
- **Sources:** drawing_-_2_point_linear_dimension.htm.
- **Selection inputs:** 2 points (snap points only; lines not selectable). Thread line points with hidden lines shown.
- **Parameters:** palette.
- **Behavior / semantics:** mode by drag direction (DR-45).
- **Edge cases & errors:** lines rejected.
- **OCCT mapping:** 2D point distance / projected on axis.
- **Priority:** P0

### DR-48 Point-to-line dimension
- **Purpose:** Distance between a point and a line.
- **Sources:** drawing_-_point_to_line_dimension.htm.
- **Selection inputs:** 1 point + 1 line (incl. thread lines).
- **Parameters:** palette.
- **Behavior / semantics:** perpendicular distance (aligned) or horizontal/vertical component by drag.
- **Edge cases & errors:** none.
- **OCCT mapping:** gp_Lin2d::Distance.
- **Priority:** P0

### DR-49 Line-to-line dimension
- **Purpose:** Distance between two parallel lines.
- **Sources:** drawing_-_line_to_line_dimension.htm, drawing_-_dimension.htm.
- **Selection inputs:** 2 lines; only lines parallel to the first highlight for second pick.
- **Parameters:** palette.
- **Behavior / semantics:** auto diameter symbol when both lines are silhouettes of same cylinder (projected and section views).
- **Edge cases & errors:** non-parallel second line not selectable.
- **OCCT mapping:** gp_Lin2d distance; silhouette ownership from HLR edge-to-face map.
- **Priority:** P0

### DR-50 Line-to-line angular dimension
- **Purpose:** Angle between two lines (interior, supplementary, vertical, conjugate/reflex).
- **Sources:** drawing_-_line_to_line_angular_dimension.htm.
- **Selection inputs:** 2 lines (incl. auxiliary-view arrow-style indicator).
- **Parameters:** palette (angular precision).
- **Behavior / semantics:** preview position between lines selects quadrant; drag grip on dimension arc across the infinite extension lines to switch to supplementary/vertical angle; also conjugate/reflex (360 - a).
- **Edge cases & errors:** parallel lines (no angle) rejected (inferred).
- **OCCT mapping:** gp_Dir2d::Angle.
- **Priority:** P0

### DR-51 3-point angular dimension
- **Purpose:** Angle from a vertex and two points.
- **Sources:** drawing_-_3_point_angular_dimension.htm.
- **Selection inputs:** vertex point, then one point on each leg.
- **Parameters:** palette.
- **Behavior / semantics:** dragging arc grip across leg lines toggles initial angle vs outside angle (360 - initial).
- **Edge cases & errors:** collinear points (inferred error).
- **OCCT mapping:** 2D vector angle.
- **Priority:** P1

### DR-52 Radial dimension
- **Purpose:** Radius of circle, arc or circular centermark.
- **Sources:** drawing_-_radial_dimension.htm. Shortcut Shift+r.
- **Selection inputs:** 1 arc / circle / centermark (circular).
- **Parameters:** palette (Toggle Radius/Diameter).
- **Behavior / semantics:** "R" prefix (inferred standard); context Foreshorten (jogged radius line when center far away; drag jog; Remove foreshorten). Works on circles partly obscured by other geometry (guide line overlay). Show dimension line inside circle per DR-14.
- **Edge cases & errors:** datum symbol cannot attach to dims convertible to foreshortened.
- **OCCT mapping:** radius from Geom_Circle of source edge (exact, from 3D) or Geom2d_Circle/ellipse (if not normal to view, true radius only when circle plane parallel to view - inferred).
- **Priority:** P0

### DR-53 Diameter dimension
- **Purpose:** Diameter of circle, arc, circular centermark, including silhouette edges.
- **Sources:** drawing_-_diameter_dimension.htm. Shortcut Shift+d.
- **Selection inputs:** 1 arc / circle / centermark.
- **Parameters:** palette.
- **Behavior / semantics:** diameter symbol included. On detail or crop views of an arc/circle it becomes a foreshortened dimension (DR-54). Obscured circle guide overlay.
- **Edge cases & errors:** as DR-52.
- **OCCT mapping:** as DR-52.
- **Priority:** P0

### DR-54 Foreshorten dimension
- **Purpose:** Half-diameter style dimension (one extension line, one arrow), common in section/detail views.
- **Sources:** drawing_-_foreshorten_dimension.htm. Shortcut Shift+h.
- **Selection inputs:** silhouette edge of a cylindrical feature, or a centerline then an arc/circle edge.
- **Parameters:** palette.
- **Behavior / semantics:** value is the diameter. Centerline+arc variant can snap to edge Min/Max point while dragging. Context: Display as direct diameter (silhouette case: value unchanged; centerline case: value recomputed, both extension lines/arrows shown) / Display as foreshorten diameter. Flip via flip-arrow grip click.
- **Edge cases & errors:** none.
- **OCCT mapping:** distance from centerline to edge x2; silhouette-to-axis via owning cylindrical face (Geom_CylindricalSurface radius).
- **Priority:** P1

### DR-55 Arc length dimension
- **Purpose:** Length of an arc.
- **Sources:** drawing_-_arc_length_dimension.htm.
- **Selection inputs:** 1 arc.
- **Parameters:** palette.
- **Behavior / semantics:** arc symbol; dragging to opposite side gives conjugate (reflex) arc length.
- **Edge cases & errors:** none.
- **OCCT mapping:** GCPnts_AbscissaPoint::Length on 3D edge (true length) when arc parallel to view.
- **Priority:** P1

### DR-56 Chamfer dimension
- **Purpose:** Note or dimension describing a chamfer.
- **Sources:** drawing_-_chamfer_dimension.htm, drawing_properties_-_dimensions.htm (Chamfer).
- **Selection inputs:** chamfered edge + adjacent edge.
- **Parameters:** style/content per DR-14 (45 deg: Note/Dimension; Angle x Length, Length x Angle, Length x Length, C Length; non-45: Dimension/Note with 3 contents; length/angle precision); prefix via palette.
- **Behavior / semantics:** computes chamfer length along adjacent edge and angle between chamfer edge and adjacent edge; 45 deg detection picks 45 style. Replicable to other views. Inspection item parsing splits length/angle (DR-20).
- **Edge cases & errors:** non-chamfer edge pair (inferred error).
- **OCCT mapping:** 2D line intersection + lengths; optionally read chamfer feature attributes.
- **Priority:** P1

### DR-57 Ordinate dimension (baseline/ordinate chains)
- **Purpose:** Groups of dimensions measured from a common datum (0) in one direction.
- **Sources:** drawing_-_ordinate_dimension.htm, drawing_properties_-_dimensions.htm.
- **Selection inputs:** first click = datum (point, edge/line, arc, centermark incl. manual); then each subsequent point in one direction; Esc ends group.
- **Parameters:** palette for group; style ANSI/ISO (DR-14).
- **Behavior / semantics:**
  - Orientation (X vs Y) chosen by moving mouse around datum before setting the 0 dimension. Second direction = new tool invocation (may reuse same datum). Each direction is its own group with single datum.
  - Group drag via middle snap moves all in sync; outermost snap moves one; jog points draggable and alignable; inferencing guidelines to (un)jog.
  - Context: Add to ordinate dimension group, Edit, Reset ordinates, Clear selection, Zoom to fit, Delete.
  - ANSI: leader lines; ISO: plus arrows connecting increasing values from origin.
  - Baseline (stacked linear from a common baseline) dimensions are not a separate tool in Onshape docs; ordinate groups serve that role.
- **Edge cases & errors:** referenced feature removed -> dimension stays red, deletable.
- **OCCT mapping:** 2D projections onto group axis.
- **Priority:** P1

### DR-58 Maximum or minimum dimension
- **Purpose:** Max or min distance between an arc/circle and another entity.
- **Sources:** drawing_-_max_or_min_dimension.htm. Shortcut Ctrl+m.
- **Selection inputs:** 1 arc/circle + 1 other entity (not curves/splines).
- **Parameters:** palette.
- **Behavior / semantics:** where the user clicks on the circle (near or far side) determines min vs max (e.g. top of top circle and bottom of bottom circle -> maximum). Related enum `DimensionMeasurementReferenceType` (AXIS, MINIMUM, MAXIMUM) in stdlib for cylinder measurements.
- **Edge cases & errors:** splines/curves excluded.
- **OCCT mapping:** center distance +/- radii; Geom2dAPI_ExtremaCurveCurve for general.
- **Priority:** P2

### DR-59 Dimension palette (text, precision, units, symbols, tolerance, dual, inspection)
- **Purpose:** Per-dimension content and tolerance editing.
- **Sources:** drawing_dimensions.htm (Dimension palette), hole_thread_callout.htm (Editing a callout).
- **Selection inputs:** one dimension (hover palette icon or double-click).
- **Parameters:**

| FS id | UI label | Type | Values / bounds | Notes |
|---|---|---|---|---|
| - | Above text | text | | Above value. |
| - | Prefix text | text | | |
| - | Count type | enum | e.g. "Count by face (Excluding patterns)" | Full list not documented; hole callouts highlight counted holes. Unavailable option on patterned holes. |
| - | Precision | enum | 0..8 decimals; "(Drawing)" links to DR-13 | |
| - | Dimension units | enum | Millimeters, Millimeters (Drawing), Centimeters, Meters, Inches, Inches fractional, Feet and inches, Feet, Yards | Sets Units + Fractional display properties; "(Drawing)" inherits. |
| - | Show/Hide units | toggle | | |
| - | Suffix text | text | | |
| - | Below text | text | | |
| - | Symbol | enum | see symbol table | Inserted at cursor. |
| - | Inspection dimension | toggle | | Oval frame around dimension. |
| - | Dual dimension | enum | single unit; single unit per drawing properties; dual | Hole callouts: top/bottom or side-by-side. |
| - | Tolerance | enum | None, Symmetrical, Deviation, Limits, Basic, MIN, MAX, Fit, Fit with tolerance, Fit (Tolerance only) | |
| - | Upper / Lower | length/angle | | Symmetrical, Deviation, Limits. +/- buttons allow +/+ and -/- for Deviation/Limits. |
| - | Fit type | enum | User defined, Clearance, Transition, Interference | Fit variants. |
| - | Hole class / Shaft class | enum | ISO/ANSI classes (e.g. H7, g6) from `fittolerancetables.gen.fs` | Fit variants. |
| - | Fit standard | enum | ANSI (Drawing), ANSI, ISO | |
| - | Fit tolerance precision | enum | 0..8 decimals | Fit with tolerance, Fit (Tolerance only). |
| - | Toggle hole/shaft class visibility | enum | Stacked without line, Stacked diagonally, Stacked with line | Fit and Fit with tolerance. Shaft class hidden by default in hole callouts/tables. |
| - | Reset text position | toggle | | |
| - | Parentheses | toggle | | Reference dimension style. |
| - | Toggle Radius/Diameter | toggle | | Radial/diameter dims. |
| - | Override precision/tolerance | bool | | Hole callouts: per-parameter override. |

  Symbol table (notes, dimensions, callouts): Degree (°, %%d), Diameter (⌀, %%c), Centerline (℄), Countersink (⌵), Depth (↧), Counterbore (⌴), Square (□), Arc length (⌒), Plus/Minus (±, %%p), Unequally disposed profile (Ⓤ), Independency (Ⓘ), Continuous feature, Spot face, Translation (▷).
- **Behavior / semantics:** rendering of tolerance: Symmetrical "v ±t"; Deviation stacked "+u / -l"; Limits stacked upper/lower limit values (nominal+upper over nominal+lower); Basic = boxed value; MIN/MAX = "v MIN"/"v MAX"; Fit = "v H7" (with optional shaft class stacked); Fit with tolerance = class plus computed deviations; Fit (Tolerance only) = computed deviations only (inferred standard rendering). Copy/paste allowed in all text boxes.
- **Edge cases & errors:** palette precision range 0..8 vs drawing properties 0..6 (doc discrepancy).
- **OCCT mapping:** none (text layout); fit tables from stdlib `fittolerancetables.gen.fs` (ISO 286 / ANSI B4.1 data).
- **Priority:** P0 (text/precision/basic tolerances) / P1 (fits, dual)

### DR-60 Tolerance model (types, defaults, MBD default tolerances)
- **Purpose:** Shared tolerance data model between features, MBD and drawings.
- **Sources:** Home/tolerance_options.htm, Library/default_tolerances_library.htm, creating_a_drawing.htm; FS `ToleranceType`, `ToleranceTypeExtended`, `ToleranceInfo`, `ToleranceFitInfo`, `PrecisionType` (toleranceTypes.fs).
- **Selection inputs:** n/a.
- **Parameters:**

| FS id | UI label | Type | Values |
|---|---|---|---|
| `ToleranceType` | Tolerance type | enum | NONE "No tolerance", SYMMETRICAL "Symmetrical", DEVIATION "Deviation", LIMITS "Limits", MIN "Min", MAX "Max", FIT "Fit"(hidden), FIT_WITH_TOLERANCE "Fit with tolerance"(hidden), FIT_TOLERANCE_ONLY "Fit (tolerance only)"(hidden), DEFAULT "Default"(hidden), BASIC (hidden, annotated "Default" in source - likely copy/paste) |
| `ToleranceInfo.upper/lower` | Upper/Lower | ValueWithUnits | Offsets from nominal, both added to nominal; upper > lower required; Symmetrical stores +t/-t. |
| `ToleranceInfo.precision` + `usePrecisionOverride` | Precision | number | |
| `ToleranceFitInfo` | standard, holeClass, shaftClass | strings | standard "ISO" or "ANSI" |
| `PrecisionType` | Precision | enum | DEFAULT "Workspace precision", ONES "0", TENTHS "0.1", HUNDREDTHS "0.12", THOUSANDTHS "0.123", TEN_THOUSANDTHS "0.1234", HUNDRED_THOUSANDTHS "0.12345", MILLIONTHS "0.123456" |

- **Behavior / semantics:** model tolerances (feature parameters, sketch dims) flow into drawings with MBD views; Default tolerance type resolves via the Part Studio's selected default tolerances library (Linear, Angular, Chamfer angles, Chamfer distances, Fillet radii, Custom class; each Constant / Precision-based / Range-based); drawing shows default tolerances with explicit ones highlighted, refreshed on update. `ToleranceSchemaClass` (FILLET_RADIUS, CHAMFER_DISTANCE, CHAMFER_ANGLE) classifies dimensions for defaults.
- **Edge cases & errors:** zero tolerances and +/+ or -/- allowed in default library.
- **OCCT mapping:** none (data model shared with Part Studio section).
- **Priority:** P1

### DR-61 Note (rich text annotation)
- **Purpose:** Single/multi-line text with optional leader(s); also used for title-block content.
- **Sources:** note.htm. Shortcut n.
- **Selection inputs:** placement: click a snap point on view geometry/face region (leader anchor) then click text location; or click empty space (no leader).
- **Parameters (Note formatting panel):**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Ruler | widget | first-line indent, left indent, right indent, bounding box width, tab stops (Left, Center, Right, Decimal) | Default tab stops at drawing intervals; drag off ruler removes tab. |
| - | Font | enum | SHX or TrueType fonts | |
| - | Text height | length | cap height (baseline to cap line) | Becomes new default. |
| - | Bold / Italic / Underline | toggle | | TrueType only. |
| - | Strikethrough | toggle | | DWG export of strikethrough needs version 2013. |
| - | Color | color | palette/hex/RGB/mixer | |
| - | Line spacing | enum | 1.0, 1.5, 2.0, 2.5, 3.0; add/remove space before/after paragraph | Whole note. |
| - | Horizontal alignment | enum | Left, Right, Center, Justified | Disabled for notes with leaders (standard governs). |
| - | List | enum | Numbers, Bullets, Uppercase letters, Lowercase letters | Restart by blank line + reselect. |
| - | Vertical alignment | enum | Top, Middle, Bottom | Relative to insertion point; disabled with leader. |
| - | Note order | enum | Top to bottom, Bottom to top | |
| - | Symbols | enum | symbol table (DR-59) | |
| - | Insert drawing property / Insert sheet reference property | field | DR-62 | |
| - | Text format | enum | All uppercase, All lowercase, Sentence case, Title case, Unmodified | For inserted property. |
| - | Date time format | enum | DR-62 | |
| - | Flag notes | bool + flag type + size | default size 2 characters | DR-63. |

- **Behavior / semantics:**
  - Text input: Ctrl+a select all; double-click word; triple-click line; system clipboard; unicode escapes `\U+00AE`; `%%d` degree, `%%c` diameter, `%%p` plus/minus; stacked fractions typed as `<n>#<n><space>` (diagonal), `<n>/<n><space>` (horizontal bar), `<n>^<n><space>` (stacked no bar); type another char instead of space to avoid conversion.
  - Bounding box grows with text; resize via square side grips; rotate via center grip (only notes without leaders; numeric angle readout). Rotating an edge-aligned note detaches it. Flip (aligned notes): rotate 180 and move to other side of edge. Drag off edge via bottom-middle grip to detach; drag onto edge to reattach.
  - Explode (context): convert to polylines on export.
  - Leaders: Add leader (attaches at grip nearest mouse), Remove leader, Add node / Remove node, Add leader from node; horizontal leader segment adjustable via node. Leader end on face shows dot (face) vs arrow (edge) (from callout behavior, inferred same).
  - Grouping: with callouts (Group annotation) or with a view (Group); note connected to a view entity is auto-grouped with the view; moving view moves note, not vice versa; Shift+drag moves one member of multi-selection.
  - Move to title block / border zones / border frame (DR-06).
  - Hover highlights attached edges blue; grouped annotations yellow.
  - Copy/paste across drawings/documents with permission.
- **Edge cases & errors:** cannot copy-paste text boxes into other text boxes (copy text instead; formatting carried).
- **OCCT mapping:** none. Text engine: TrueType shaping (e.g. rustybuzz + ttf-parser), SHX font parser (custom; required for AutoCAD compatibility), layout with tabs/indents.
- **Priority:** P0

### DR-62 Property fields and date formats
- **Purpose:** Live metadata fields inside notes, title blocks, callouts and tables.
- **Sources:** note.htm (Insert drawing property, Insert sheet reference property, Editing a field in the title block), sheets.htm.
- **Selection inputs:** cursor position in note editor.
- **Parameters:**

| FS id | Field group | Fields |
|---|---|---|
| - | Drawing property | Drawing created by, Drawing created date, Drawing description, Drawing last changed by, Drawing last changed date, Drawing name, Drawing part number, Drawing revision, Drawing Title 1, Drawing Title 2, Drawing Title 3, Sheet number, Total sheets, Sheet scale, Sheet size |
| - | Sheet reference property (entity the sheet's views reference) | Name, Description, Part Number, Revision, State, Vendor, Project, Product Line, Title 1, Title 2, Title 3, Material, Mass, + active custom properties (alphabetical at bottom) |
| - | Text format | All uppercase, All lowercase, Sentence case, Title case, Unmodified |
| - | Date format codes | yyyy-M-d, yyyy-MM-dd, M.d.yyyy, M/d/yyyy, M/d/yy, MM-yy, MM/dd/yyyy, MMM.d, yy, MMMM yy, MMMM d,yyyy, d-MMM-yy, d MMMM yy, dd.MM.yyyy, dd/MM/yyyy, dddd,MMMM d,yyyy, dd-mm-yyyy, d-MMM-yyyy, dd/mm/yy, dd/mm/yyyy, dd.mm.yy, dd.mm.yyyy (help's sample values for lowercase "mm" variants look like day-month and are probably doc errors) |

- **Behavior / semantics:** fields render with gray background; resolved values update live; UTC used for dates; empty -> "----" placeholder (hideable).
- **Edge cases & errors:** "Sheet reference" when sheet references multiple entities: first/primary reference (inferred).
- **OCCT mapping:** none (template-string evaluator).
- **Priority:** P0

### DR-63 Flag notes
- **Purpose:** Numbered flags on list items in notes, referenceable from callouts.
- **Sources:** note.htm (Creating a flag on a note), callout_balloon.htm (Flag note property), drawing_properties_-_annotations.htm.
- **Selection inputs:** list-formatted note items.
- **Parameters:** Flag notes (bool per item), flag type (border shape), character size (default 2).
- **Behavior / semantics:** only bulleted/numbered/lettered list items can be flagged; callout can display a flag note property to point to that item.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P2

### DR-64 Leaders, attachment and annotation grouping (shared annotation behavior)
- **Purpose:** Common leader/attachment/group model for notes, callouts, GD&T, datums, surface finish, weld, hole callouts.
- **Sources:** note.htm, callout_balloon.htm, geometric_tolerance.htm, surface_finish.htm, weld_symbol.htm, datum.htm.
- **Selection inputs:** annotation(s).
- **Parameters:** context commands: Add leader, Remove leader(s), Add node, Remove node, Group annotation / Remove annotation from group, Group (with view) / Remove from group, Align horizontally / Align vertically (callouts), Move to (layer), Copy, Paste, Paste as table (GD&T), Edit.
- **Behavior / semantics:**
  - Attachment targets: view edges, points, face regions (dot terminator), holes, centermarks (auto and manual), dimensions (snap; follows dimension), dimension extension lines, GD&T frame edges (for datums), free space.
  - GD&T dropped on a dimension snaps/groups and moves with it; dragging GD&T off a dimension ungroups and adds leader in line with dimension.
  - Callouts can group with views, callouts, dimensions, welds, surface finishes, datums (drag onto target until highlight).
  - Grouped with view: moving view moves annotations; moving annotation does not move view.
- **Edge cases & errors:** annotation connected to one view only.
- **OCCT mapping:** none (2D annotation graph; attachment = reference to (view, entity persistent id, parameter) or to annotation id).
- **Priority:** P0

### DR-65 Callout (balloon), BOM-linked and stacked
- **Purpose:** Balloon with leader showing part/BOM/cut-list property (usually Item No.).
- **Sources:** callout_balloon.htm, bom_table.htm.
- **Selection inputs:** leader anchor (view face/edge/point/centermark/dimension) + placement click; or free placement.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Part property | enum | all part properties | Associative to part metadata. |
| - | BOM table property | enum | BOM table columns | Default Item No. when BOM present. |
| - | Instance property | enum | Reference designator | |
| - | Flag note property | ref | note flags | |
| - | Cut list table property | enum | cut list columns | |
| - | Text fields | text x4 | above, below, left, right | |
| - | Insert symbol | enum | symbols | |
| - | Text height | length | | |
| - | Border shape | enum | shapes incl. None (circle default, inferred); horizontal leader node only when None | |
| - | Border size | int | characters wide | |

- **Behavior / semantics:** associative to BOM/cut list; when table order changes use Update drawing to resync numbers. Leader terminator: dot on faces, arrow on edges. Align horizontally/vertically for multi-select (no persistent lock). Hovering a BOM row highlights its callouts. Stacked callouts (assemblies with BOM only): Add stacked callouts -> Stacked instances (pick instances in view or "Add instances from BOM" by clicking BOM first column) -> stacked by Stack direction/gap (DR-15/21). Deleting a dimension deletes its grouped callout.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** view entity -> part instance mapping from HLR edge provenance.
- **Priority:** P0

### DR-66 Datum feature symbol
- **Purpose:** Associative datum symbol (letter in box with triangle) on an edge representing a surface.
- **Sources:** datum.htm.
- **Selection inputs:** edge (linear or circular) of a part view; or GD&T frame edge (red crosshair); dimension leader/extension lines; hole callouts; radius/diameter dims.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Label | text | auto-increment letter A, B, C... | Part Studio MBD: one or more uppercase letters followed by optional digits, max 7 chars. |

- **Behavior / semantics:** click edge, drag away to set datum line length, click to place; tool stays active for multiple. Drag after placement; extension adjusts. Occluded dimension arrows/lines hidden. Styling: Font, Text height, Color, Datum size (triangle arrowhead type via Annotations Arrowhead).
- **Edge cases & errors:** cannot attach to radius/diameter dims that can be foreshortened (e.g. in detail view).
- **OCCT mapping:** none.
- **Priority:** P1

### DR-67 Datum target
- **Purpose:** Datum target symbols (point, circle, rectangle, custom area) with locating dimensions.
- **Sources:** datum.htm (Creating a datum target).
- **Selection inputs:** target location on view; origin point; optional reference line edge; X/Y direction edges.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | Target area | enum | Point (X), Point (dot), Circle, Rectangle, Custom (any closed area) | |
| - | Style | enum | target callout styles | |
| - | Area size | bool + length(s) | Point: optional diameter; Circle: diameter; Rectangle: side lengths; Custom: n/a | Precision dropdown. |
| - | Label | text | A1, A2... (new targets increment digit; new datum increments letter) | |
| - | Location mode | enum | Length (X-Position, Y-Position from origin; X/Y flip arrows), Angle (Diameter distance, Angle, Radial lines, Reference line, Flip, Rotate) | Origin not for Custom. |
| - | Instance | int | evenly spaced over 360 deg; Opposite direction (CCW) | |
| - | Targets | list | | Add more targets with same letter. |

- **Behavior / semantics:** generates locating dimensions; context Hide dimension; Show dimensions (This target / All targets). Styling adds hatch pattern and hatch scale. Selecting one target highlights series yellow.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P2

### DR-68 Geometric tolerance (feature control frame builder)
- **Purpose:** GD&T feature control frames, single or composite, with datums and modifiers.
- **Sources:** geometric_tolerance.htm; FS enums `GTolConstraintType`, `GTolExtendedConstraintType` (gtolconstrainttype.gen.fs, gtolextendedconstrainttype.gen.fs).
- **Selection inputs:** attach to edges, holes, dimensions (with/without leader), dimension extension lines, extension lines, surface regions, centermarks, or free space.
- **Parameters:**

| FS id | UI label | Type | Values / bounds | Notes |
|---|---|---|---|---|
| - | Upper text | text | max 50 chars | |
| - | Lower text | text | max 200 chars | |
| - | Prefix / Suffix | text | max 20 chars each | Left/right of frame. |
| `GTolConstraintType` | Symbol | enum | TRUE_POSITION Position, CONCENTRICITY, SYMMETRY, PARALLELISM, PERPENDICULARITY, ANGULARITY, CYLINDRICITY, FLATNESS, ROUNDNESS, STRAIGHTNESS, PROFILE_SURFACE "Profile (Surface)", PROFILE_LINE "Profile (Line)", CIRCULAR_RUNOUT, TOTAL_RUNOUT | Characteristic types: location (position, concentricity, symmetry), orientation (parallelism, perpendicularity, angularity), form (cylindricity, flatness, roundness, straightness), profile, runout. Symbols table also lists "Square". |
| `GTolExtendedConstraintType.COMPOSITE_FRAME` | Composite | enum value | available on 2nd+ frame | Spans symbol across frames. |
| - | Tolerance value | text | with left/right symbol fields, each max 7 symbols | Symbols allowed depend on symbol type and field. |
| - | Datums 1-3 | text | must start with uppercase letter; modifiers via palette; hyphens allowed (e.g. A-B) | |
| - | Add frame / Remove last frame | button | up to 5 frames | Move up/down / Remove via right-click on added rows. |
| - | Leader | enum | Leader, All around, All over | |
| - | Show symbols palette | toggle | Diameter (Ø), Spherical diameter (SØ), Radius (R), Spherical radius (SR), Controlled radius (CR); modifiers MMC Ⓜ, LMC Ⓛ, RFS Ⓢ, Free state Ⓕ, Tangent plane Ⓣ, Unequally disposed Ⓤ, Projected tolerance zone Ⓟ, Statistical ⟨ST⟩, Continuous feature, Spot face, Translation | |

- **Behavior / semantics:** tool stays active for multiple placements. Leader with movable node; Add leader for more. Placed on dimension: snaps and follows; drag off -> unsnapped with leader inline. Context: Edit, Paste, Copy, Paste as table, Add leader, Remove leaders, Move to, Clear selection, Zoom to fit, Delete. Styling: Font, Text height, Color, Arrowhead. Inference points/lines for alignment. Part Studio MBD: only one GD&T per dimension, many per face; invalid field entries highlighted red with tooltip.
- **Edge cases & errors:** field length overflow -> red field.
- **OCCT mapping:** none (annotation rendering with GD&T symbol font/glyphs; XCAF `XCAFDimTolObjects_GeomToleranceObject` exists for STEP AP242 PMI if needed for MBD export).
- **Priority:** P1

### DR-69 Surface finish symbol
- **Purpose:** Surface texture symbols (ANSI/ISO).
- **Sources:** surface_finish.htm.
- **Selection inputs:** white space, point on edge of circular view, view region on a part face, or dimension extension line.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | Type | enum | Basic, Material removal required, Material removal prohibited, ISO Basic, ISO material removal required, ISO material removal prohibited | |
| - | All around | bool | | |
| - | Parenthesis | bool | | |
| - | Direction lay | enum | lay symbols (=, ⊥, X, M, C, R, P - inferred standard set) | |
| - | Data cells | text | symbol data fields (roughness values, process, etc.) | Layout per standard (ISO 1302 / ASME Y14.36, inferred). |
| - | Add row | button | up to 5 additional rows | |

- **Behavior / semantics:** multiple symbols placed in one session; leaders added via context; groupable with callouts. Context: Edit, Copy, Add leader, Remove leaders, Move to, Clear selection, Zoom to fit, Delete. Styling: Font, Text height, Color, Arrowhead.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-70 Weld symbol
- **Purpose:** AWS/ANSI and ISO weld symbols.
- **Sources:** weld_symbol.htm, PartStudio/inspection_table.htm (Weld symbol fields); FS enums `WeldStandard`, `WeldType`, `WeldJointType`, `WeldContourType`, `WeldFinishing`, `FieldWeldFlag`.
- **Selection inputs:** line or vertex in view (arrow attachment).
- **Parameters:**

| FS id | UI label | Type | Values / bounds | Notes |
|---|---|---|---|---|
| `WeldStandard` | Standard | enum | ANSI, ISO | Default = drawing standard or Annotations > Weld symbols setting. Dialog layout differs by standard. |
| - | Flip identification line | bool | | ISO only (dashed identification line). |
| `WeldJointType` | Joint spacer | enum | Double V groove, Double bevel groove, Double U groove, Double J groove, Double flare V, Double flare bevel (NONE) | Alternative to per-side joint types. |
| `WeldType` | Weld type (upper / lower) | enum | None, Square groove, V groove, U groove, J groove, Bevel groove, Single bevel butt broad root, V flare groove, Bevel flare groove, Bead joint, Fillet joint, Plug or Slot joint, Seam joint, Spot joint | Drawing help also lists Seam centered joint and Spot centered joint (not in FS enum). |
| - | Groove angle (upper/lower) | angle | 0..360 deg | |
| - | Root opening (upper/lower) | length | | |
| `WeldFinishing` | Finishing (upper/lower) | enum | None, C Chipping, G Grinding, H Hammering, M Machining, R Rolling, U Unspecified | ANSI only. |
| `WeldContourType` | Contour (upper/lower) | enum | None, Flat, Convex, Concave | |
| - | Weld size / depth / length / pitch (upper/lower) | length | | |
| - | Stagger | bool | | All ANSI/ISO joints when Symmetric or top=bottom. |
| - | Symmetric | bool | | Not for ANSI joint spacers. |
| - | Second fillet | bool | | Certain joints. |
| - | Reference | bool + text | max 100 chars (Part Studio) | Tail text. |
| - | All around | bool | | Circle at arrow/reference junction. |
| `FieldWeldFlag` | Field weld flag | enum | NONE, UPPER, LOWER | |

- **Behavior / semantics:** additional leaders via context; leader nodes; inferencing; groupable with callouts; styling Font, Text height, Color, Arrowhead. Upper = other side, lower = arrow side (ANSI convention, inferred).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** none (symbol library rendering).
- **Priority:** P2

### DR-71 Hole / thread callout
- **Purpose:** Callout auto-filled from Hole or External thread feature metadata.
- **Sources:** hole_thread_callout.htm, drawing_properties_-_annotations.htm (Hole callouts).
- **Selection inputs:** circle belonging to a Hole feature, or thread of an External thread feature; additional leaders to same-size holes.
- **Parameters:** palette as DR-59 (Above/Prefix/Count type/Units/Suffix/Below/Symbol/Inspection/Dual/Override precision/tolerance with Precision, Tolerance types, Upper/Lower, Fit type, Hole class, Fit tolerance precision, Fit standard ANSI (Drawing)/ANSI/ISO, Toggle shaft class visibility).
- **Behavior / semantics:** text generated from hole spec (e.g. count x Ø diameter ↧ depth, ⌴ / ⌵ counterbore/countersink dia/depth/angle, thread designation and depth); precision, tolerance, fit class and shaft class taken from the feature by default; Count type counts holes in feature/pattern (counted holes highlighted); dual units top/bottom or side-by-side. Replicate annotations copies to other views. Styling: font, text height, color, arrowhead, text justification. Also applies to sheet metal flat-pattern holes.
- **Edge cases & errors:** "Count by face (Excluding patterns)" unavailable for patterned holes. Leaders only to same-size holes.
- **OCCT mapping:** requires hole feature attributes on faces (noshape modeling layer: attach hole spec attribute to generated faces, like Onshape `holeAttribute`), not OCCT.
- **Priority:** P1

### DR-72 Centerlines (2-point, edge-to-edge, 3-point circle, 2-point circle)
- **Purpose:** Manual centerline construction geometry.
- **Sources:** drawing_construction_tools.htm, drawing_properties_-_construction_geometry.htm.
- **Selection inputs:** per tool below.
- **Parameters:**

| FS id | Tool | Inputs | Notes |
|---|---|---|---|
| - | 2 point centerline | 2 points (snap optional; can reuse other centerline endpoints) | |
| - | Edge-to-edge centerline | single cylindrical/conical silhouette edge, or Shift+select two edges / two concentric arcs | Midline between edges. |
| - | 3 point circle centerline | 3 points (hole centers, end/mid/quad) | Bolt-circle; dimensionable. |
| - | 2 point circle centerline | center point + circumference point | |

- **Behavior / semantics:** associative to references; end grips change length (can be shorter than references); snap point drag moves. Delete via select + Delete. Auto centerlines for holes/cylinders/spheres controlled per view (DR-38) and by Insert view defaults.
- **Edge cases & errors:** none.
- **OCCT mapping:** 2D construction (gp_Lin2d midline, gce_MakeCirc2d from 3 points).
- **Priority:** P1

### DR-73 Centermarks (single, circular, linear; automatic)
- **Purpose:** Center marks on circles/arcs and patterns.
- **Sources:** drawing_construction_tools.htm (Centermark), view_context_menu.htm (Show/hide auto centermarks), drawing_properties_-_construction_geometry.htm.
- **Selection inputs:** circle/arc edges (also round/elliptical faces not normal to view).
- **Parameters:**

| FS id | UI label | Type | Values |
|---|---|---|---|
| - | Type | enum | Single, Circular, Linear |
| - | Circular options | bools | Reference center, Bolt circle, Radial lines, Pattern centermark |
| - | Linear options | bool | Connecting lines |

- **Behavior / semantics:** automatic centermarks for Hole features when view is normal to hole face, incl. patterned/mirrored holes (circular/linear patterns get connecting lines). Align centermark (context) orients to a picked line/edge. Extended lines and grips per DR-17. Automatic/implicit centermarks are copied with sheet duplicate / view copy / move. Centermarks are dimension and annotation targets.
- **Edge cases & errors:** dangling possible.
- **OCCT mapping:** circle centers from Geom_Circle of source edges projected; pattern detection from feature attributes.
- **Priority:** P1

### DR-74 Virtual sharp
- **Purpose:** Associative intersection point of two linear edges (e.g. across a fillet/chamfer) for dimensioning.
- **Sources:** drawing_construction_tools.htm (Virtual sharp), drawing_properties_-_construction_geometry.htm.
- **Selection inputs:** 2 linear edges.
- **Parameters:** visual style Centermark or Edge extension (Drawing properties).
- **Behavior / semantics:** fully associative; dimensions attach only to the intersection point.
- **Edge cases & errors:** parallel edges (no intersection; inferred error).
- **OCCT mapping:** IntAna2d_AnaIntersection of two gp_Lin2d.
- **Priority:** P2

### DR-75 Insert image
- **Purpose:** Place raster images (logos, QR/bar codes).
- **Sources:** drawing_insert_image.htm.
- **Selection inputs:** image file tab in document (or other documents via Browse documents; or Import).
- **Parameters:** image file (JPG, PNG, GIF); placement by click-drag (aspect ratio kept); Properties: sheet, references.
- **Behavior / semantics:** images from other documents are copied (linked; "Open linked document" context). Borders hidden by default; Show/Hide image borders affects all images. Deleting the image tab removes image on next update; images inserted into drawings imported from Documents page are non-linked.
- **Edge cases & errors:** none.
- **OCCT mapping:** none (image decode via `image` crate; embed in PDF/DWG export).
- **Priority:** P1

### DR-76 Inspection items (balloons for inspection)
- **Purpose:** Numbered inspection markers on dimensions/annotations for quality inspection reports.
- **Sources:** inspection_item.htm, drawing_properties_-_inspection.htm.
- **Selection inputs:** manual: hover annotation/callout and click; automatic: scope.
- **Parameters:**

| FS id | UI label | Type | Values | Notes |
|---|---|---|---|---|
| - | Mode | enum | Manual, Automatic | |
| - | Appearance | style | font, text, border shape, border size, border color, solid fill | Defaults from DR-20. |
| - | Scope | enum | Entire drawing, Sheet, View (pick) | Automatic only; Preview button. |
| - | Number | int | | Reorder mode. |

- **Behavior / semantics:** numbers increment; attach to dimensions, ordinate dims, tolerances (per frame), surface finish, welds, hole callouts, callouts, notes, title block areas; not datums. Automatic adds to dimensions, ordinate dims, welds, surface finish, GD&T, callouts; never basic or reference dimensions; callouts excludable. Dual dims: apply to primary only. Context: Edit, Reorder inspection items (type number, click items sequentially; auto-increments), Remove, Remove all, Clear selection, Zoom to fit, Delete. Released drawings: reusing previous numbers gives warning. Hide inspection items toolbar toggle; export option hides them.
- **Edge cases & errors:** reuse warning in released drawings.
- **OCCT mapping:** none.
- **Priority:** P2

### DR-77 Drawing inspection table and CSV exports (Inspection list, Drawing definition list)
- **Purpose:** Tabular view of datums and inspection items; export for QA.
- **Sources:** inspection_item.htm (Viewing the Inspection table, Exporting), PartStudio/inspection_table.htm, exporting_a_drawing.htm.
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values |
|---|---|---|---|
| - | View | enum | All sheets, Current sheet |
| - | Export to CSV | command | opens Export Drawing dialog with Format "Inspection list" or "Drawing definition list" |
| - | Assign number / Remove inspection item / Remove all / edit number | row context | |
| - | Template columns | admin setting | add/hide/reorder columns (company preference) |

- **Behavior / semantics:** includes MBD data brought into views and default tolerances (refreshed on update). Hover row cross-highlights reference. Inspection list CSV columns: Inspection item number, Sheet number, Annotation type, Unit, Nominal value, Tolerance, Upper limit, Lower limit; only assigned items exported (otherwise header only). Drawing definition list CSV = all drawing annotations. Inspection-list export requires an export rule preference (Onshape plan setting).
- **Edge cases & errors:** spreadsheets may reformat symbols.
- **OCCT mapping:** none.
- **Priority:** P2

### DR-78 MBD status panel and model annotations in views
- **Purpose:** Track Part Studio MBD annotations not shown in drawing views.
- **Sources:** mbd_status.htm, creating_a_drawing.htm, views.htm (Show annotations from model).
- **Selection inputs:** none.
- **Parameters:**

| FS id | UI label | Type | Values |
|---|---|---|---|
| - | Model | enum | Current sheet, All Models, Part |
| - | Datums table / Characteristics table | lists | missing items |
| - | Show (row context) | command | restore deleted annotation |

- **Behavior / semantics:** badge with count of missing references on panel icon; checkmark when all present; message with count. Deleted MBD annotations turn gray in table and can be shown again. MBD annotations appear in views whose plane matches annotation plane (with duplicate prevention, DR-02). Show annotations from model per view toggles them.
- **Edge cases & errors:** none.
- **OCCT mapping:** none (requires noshape MBD model; annotation plane matching to view gp_Ax2 normal).
- **Priority:** P2

### DR-79 Drawing sketch tools
- **Purpose:** 2D sheet geometry not tied to the model (with optional constraints to view geometry).
- **Sources:** drawing_sketch_tools.htm, styles.htm (Sketch geometry styles).
- **Selection inputs:** clicks on sheet; snaps to view geometry and sketch entities.
- **Parameters:**

| FS id | Tool | Shortcut | Inputs / notes |
|---|---|---|---|
| - | Line | l | Click start, click successive segments (each segment separate entity), Esc ends; H/V inferencing. |
| - | Center point circle | c | Center, radius. |
| - | 3 point circle | | 3 circumference points. |
| - | Tangent arc | | Pick line/edge end (start, tangent), click end. |
| - | Corner rectangle | g | Two diagonal corners. |
| - | Center point rectangle | r | Center, corner. |
| - | Spline | | Click fit points; double-click or Esc ends; drag points to reshape. |
| - | Spline point | | Add points on spline. |
| - | Sketch point | | Place point. |
| - | Hatch region | | DR-37. |
| - | Trim | m | Click curve segment to trim to nearest intersections/bounds; if none, whole curve deleted; clicking points deletes them. |

- **Behavior / semantics:** negative dimension values flip direction; styles via Styles panel (linestyle, pitch scale, thickness, color). Sketch dimensions on drawing sketches drive geometry (driven-dimension color in DR-17, inferred that Dimension tool can drive drawing sketch entities).
- **Edge cases & errors:** none.
- **OCCT mapping:** Geom2d curves; Trim via Geom2dAPI_InterCurveCurve; spline via Geom2dAPI_Interpolate.
- **Priority:** P1

### DR-80 Drawing sketch constraints
- **Purpose:** Geometric constraints among drawing sketch entities and to view geometry.
- **Sources:** working_with_drawing_constraints.htm, drawings.htm (shortcuts).
- **Selection inputs:** sketch entities (+ view edges as references); pre-select then click constraint, or constraint then pick.
- **Parameters:**

| FS id | Constraint | Shortcut | Applies to |
|---|---|---|---|
| - | Horizontal | h | 1+ lines or set of points |
| - | Vertical | v | 1+ lines or set of points |
| - | Coincident | i | 2+ entities |
| - | Perpendicular | Shift+l | 2 lines |
| - | Parallel | b | 2+ lines |
| - | Tangent | | line/curve pairs |

- **Behavior / semantics:** only on drawing sketch tools (not construction tools). Hover shows constraint icon with yellow background and orange referenced entities; Shift while hovering keeps all visible; view context Show/hide constraints; delete via icon + Delete. Unsolvable constraints shown red. Inferencing auto-creates H/V only.
- **Edge cases & errors:** over-constrained -> red.
- **OCCT mapping:** none; reuse noshape sketch solver (2D GCS; e.g. planegcs-style) with view geometry as fixed references.
- **Priority:** P2

### DR-81 Table framework (shared editing for all drawing tables)
- **Purpose:** Common table object: placement anchor, sizing, cell formatting, split, copy, move.
- **Sources:** table.htm, bom_table.htm, drawing_hole_table.htm, revision_table.htm, custom_table.htm, cut_list_table.htm; FS `TableTextAlignment` (LEFT, CENTER, RIGHT).
- **Selection inputs:** table edge (whole table), cells (click; Shift+click range; drag-select yellow outline).
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Fixed corner | enum | Upper right (default), Upper left, Lower left, Lower right | Anchor; table grows away from it. Double-click fixed corner (or any corner) opens table properties. |
| - | Order | enum | Top to bottom, Bottom to top | BOM/hole/revision/custom/cut list. Bottom to top puts header at bottom. |
| - | Resize (cell) | dialog | Column width, Row height | Cannot exceed sheet size. |
| - | Size > Columns equally / Rows equally | command | | Average width/height of selected. |
| - | Cell formatting panel | panel | Note text formatting (font, size, bold, italic, underline, strikethrough, line spacing, vertical/horizontal alignment) | |
| `TableTextAlignment` | column alignment | enum | LEFT, CENTER, RIGHT | For FS custom tables. |

- **Behavior / semantics:** corner grips move; mid-edge grips resize proportionally; double-click mid-point auto-fits to content; double-click column/row divider fits that column/row; drag top/right cell edge resizes both neighbors (table size constant); drag bottom/left edge resizes that row/column (table resizes). Table corner snaps to sheet corner (red snap). Context (whole table): <type> table properties, Split table above/below (-> two tables; Move to sheet), Copy, Move to (layers), Bring to front, Send to back, Move to sheet, Clear selection, Zoom to fit, Delete. Copy cells to clipboard as spreadsheet data (tab-separated/HTML) and paste into drawings as new table. Date properties in cells use UTC. Hover over rows of associative tables cross-highlights geometry and callouts.
- **Edge cases & errors:** none.
- **OCCT mapping:** none (2D table layout engine).
- **Priority:** P0

### DR-82 General (custom-content) table and paste-from-spreadsheet
- **Purpose:** Free-form editable tables.
- **Sources:** table.htm.
- **Selection inputs:** placement click.
- **Parameters:**

| FS id | UI label | Type | Values |
|---|---|---|---|
| - | Rows / Columns | int | counts |
| - | Title row | bool | row spanning all columns at top |
| - | Header row | bool | row below title |
| - | Fixed corner | enum | DR-81 |

- **Behavior / semantics:** after placement Note panel opens; Tab/Enter next cell, Shift+Tab previous, Alt+Enter newline in cell. Cell panel: Insert row above/below, Insert column left/right, Remove rows/columns, Duplicate rows/columns, Size columns/rows equally, Merge cells (horizontal, vertical, all), Unmerge. Paste as table (from Excel/Google Sheets): placed with top-left at cursor; merged cells preserved; preserved formatting: text size, bold/italic/underline/strikethrough (only if whole cell), cell width/height; not preserved: background, text color, hyperlinks; content center-aligned; cannot paste into existing table; clipboard consumed after paste.
- **Edge cases & errors:** some special characters unsupported.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-83 BOM table
- **Purpose:** Insert an associative bill of materials from an assembly BOM (or file/app data).
- **Sources:** bom_table.htm, callout_balloon.htm, Assembly/bill_of_material.htm.
- **Selection inputs:** BOM source in Insert dialog.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Source | enum | Onshape BOM of an assembly (current document / other documents; versions, workspaces, releases; Release filter), BOM data (uploaded file; Import), BOM app (app store) | Assemblies in document listed in dropdown. |
| - | Template | enum | company/enterprise BOM templates | Visible if templates exist. |
| - | BOM type | enum | Flattened, Structured - Top level, Structured - Multi-level | BOM table properties dialog lists Flattened / Structured. |
| - | Order | enum | Top to bottom (default), Bottom to top | |
| - | Fixed corner | enum | default upper right | |

- **Behavior / semantics:**
  - Flattened: parts only, no hierarchy. Top level: one level of hierarchy (subassemblies as single rows). Multi-level: indented item numbers 1, 1.1, 1.2, 2, 2.1...
  - Rows/item numbers follow assembly Instances list order; columns = assembly BOM columns (default Item, Quantity, Part number, Description) or template; Subassembly BOM behavior property (Show assembly and components / Show assembly only / Show components only) governs appearance.
  - Structured item numbering works when view reference differs from BOM reference as long as one contains the other.
  - Callouts default to Item No. and update with Update drawing after reorder.
  - Hover row: highlights its callouts (blue) and geometry in all views.
  - Context: BOM table properties (Reference link, BOM type, Order, Template, fixed corner), Split BOM table below, Copy, Move to, Bring to front, Send to back, Switch to <assembly>, Move to sheet, Clear selection, Zoom to fit, Delete. Sheets panel: Properties, Switch to BOM, Delete, Replace (choose another assembly; associated views also re-referenced).
  - Cell Override checkbox: replaces cell text locally (black triangle indicator + tooltip); requires the line item to have a Part Number; multi-select hides override.
  - Template option "All uppercase in Drawings" forces uppercase headings/strings.
- **Edge cases & errors:** override disabled when no part number.
- **OCCT mapping:** none (BOM from noshape assembly model).
- **Priority:** P0

### DR-84 Assembly BOM source model (columns, templates, subassembly behavior)
- **Purpose:** Define the data the drawing BOM shows.
- **Sources:** Assembly/bill_of_material.htm, Assembly/bom_table_formatting.htm, Assembly/bom_table_templates.htm.
- **Selection inputs:** n/a.
- **Parameters:**

| FS id | UI label | Type | Values |
|---|---|---|---|
| - | BOM type | enum | Flattened, Structured |
| - | Columns | list | Item (fixed, required), Quantity, Part number, Description (defaults) + any property (custom properties, Vendor, Material, ...) ; Move left/right; Remove (except Item); resize |
| - | Sort | toggle | per column: unsorted -> ascending -> descending (not Item; one column at a time) |
| - | Insert items | rows | non-geometric items (glue, paint...) |
| - | Suppress from this BOM / Show excluded/suppressed | row state | suppressed shown with dash item number |
| - | Show top level assembly row | bool | aggregated values row at bottom |
| - | Template (Create BOM template) | dialog | Name (required), BOM Type, Property columns (Item required), All uppercase in Drawings, Show top level assembly row |
| - | Subassembly BOM behavior (assembly property) | enum | Show assembly and components (default), Show assembly only, Show components only |

- **Behavior / semantics:** editing cells writes back to part/assembly properties (where editable); released parts read-only; Generate missing part numbers (sequential scheme); Copy table / Export to CSV.
- **Edge cases & errors:** multivalued Category can't be a column.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-85 Hole table
- **Purpose:** Tabulate holes of a part view with tags and X/Y locations from an origin.
- **Sources:** drawing_hole_table.htm, PartStudio/hole_table.htm, drawing_properties_-_tables.htm.
- **Selection inputs:** view(s) of a single part (not whole Part Studio); origin point; axis edge; excluded holes.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Show location data in view(s) | bool | | Tags/axes drawn in view. |
| - | Location precision | enum | decimals | |
| - | Views | selection | 1+ views (click field again for more) | Part drawings only. |
| - | Origin | point | | |
| - | X or Y axis | edge | | Axis direction; Flip X / Flip Y on axis context. |
| - | Excluded holes | selection | | |
| - | Order | enum | alphanumeric Top to bottom / Bottom to top | |
| - | Fixed corner | enum | | |
| - | Collapse tags | bool | | Properties dialog. |

- **Behavior / semantics:** preview table populates as views selected; rows: tag (e.g. A1, A2 per hole type, inferred), X, Y, size/description from hole feature (diameter, depth, cbore/csink, thread; shaft fit class hidden by default); hover row cross-highlights holes. Hole tag context: Hole table properties, Flip X direction, Flip Y direction, Clear selection, Zoom to fit. Table context: properties, Split hole table above/below, Move to, Bring to front, Send to back, Move to sheet, Delete. Indicator style ANSI/ISO.
- **Edge cases & errors:** whole-Part-Studio views not selectable.
- **OCCT mapping:** hole identification from feature attributes; coordinates = projected hole axis points in view CS relative to origin/axis.
- **Priority:** P1

### DR-86 Revision table and revision callouts
- **Purpose:** Table of drawing revisions from release management, plus revision callouts.
- **Sources:** revision_table.htm.
- **Selection inputs:** placement click.
- **Parameters:**

| FS id | UI label | Type | Values / default | Notes |
|---|---|---|---|---|
| - | Multi-sheet | enum | Linked (full table on all sheets), See sheet 1 (full table on insertion sheet, small "See sheet 1" table elsewhere) | |
| - | Visible rows | int | number of revisions listed | |
| - | Order | enum | Top to bottom (newest at bottom), Bottom to top (newest at top) | |
| - | Hide revision callouts no longer shown in the table | bool | | |
| - | Fixed corner | enum | | |

- **Behavior / semantics:** requires release management enabled; rows update automatically through release workflow; pending revision row highlighted blue with unfilled triangle on label and "*" suffix (customizable); description of pending revision editable by double-click (only admin edits released). Format governed by company Revision table template. Revision callouts: context "Add revision callout" (sheet or table row), Hide/Show revision callouts per row; groupable with dimensions. Column headers and title editable.
- **Edge cases & errors:** UTC dates.
- **OCCT mapping:** none (needs noshape revision/release data; if no PDM, allow manual revision rows - noshape design choice).
- **Priority:** P2

### DR-87 Custom table (FeatureScript-defined Part Studio table)
- **Purpose:** Insert a table computed by a FeatureScript custom table definition over a Part Studio.
- **Sources:** custom_table.htm; FS `defineTable`, `table`, `tableColumnDefinition`, `tableRow`, `TableTextAlignment` (table.fs).
- **Selection inputs:** Part Studio (Add), custom table definition (Insert -> Select a custom table).
- **Parameters:** Part Studio, custom table, Order, Fixed corner. Properties dialog: Document, Workspace, Custom table (link), Feature Studio (link + version), Sheet, Order, Fixed corner.
- **Behavior / semantics:** data covers the entire Part Studio (not just parts in the drawing); updates with Part Studio on drawing update; cells editable via Note panel (formatting); same formatting/resizing as DR-81.
- **Edge cases & errors:** FS table errors per cell (`tableCellError`).
- **OCCT mapping:** none (depends on noshape scripting/custom-feature system; P2).
- **Priority:** P2

### DR-88 Cut list table
- **Purpose:** Insert frame cut list of a composite part.
- **Sources:** cut_list_table.htm; FS `cutlist` feature.
- **Selection inputs:** cut list (composite part created by Cut list after Frame) via Insert.
- **Parameters:** Insert source, Fixed corner; properties: Sheet, Order, Fixed corner.
- **Behavior / semantics:** hover row highlights callouts and geometry; callouts can show cut list properties; Split cut list table below / Unsplit; Change configuration (affects table only, not views); other table commands per DR-81. View properties "Cut list table reference".
- **Edge cases & errors:** requires cut list to exist in Part Studio.
- **OCCT mapping:** none.
- **Priority:** P2

### DR-89 Insert DXF or DWG into a drawing
- **Purpose:** Copy model-space wireframe and text from a DXF/DWG file into the sheet as native entities.
- **Sources:** insert_dxf_or_dwg.htm, custom_drawing_templates.htm.
- **Selection inputs:** DXF/DWG file tab (search, Other documents, Import).
- **Parameters:** file; placement (inferred click).
- **Behavior / semantics:** only model space; only wireframe (lines, arcs, polylines, etc.) and notes; no 3D; all colors removed (layer default color applied); blocks exploded; polylines exploded; simple notes -> editable Notes; from other documents the file is copied. Use Move to layers to build templates. Fonts may need replacing with supported fonts.
- **Edge cases & errors:** unsupported entities dropped (inferred).
- **OCCT mapping:** OCCT has no DWG reader and no open-source DXF reader in core (DXF exists only as a commercial OCCT component); use a DXF/DWG library (DR-95).
- **Priority:** P1 (DXF) / P2 (DWG)

### DR-90 Importing a drawing (DWG/DXF -> drawing tab)
- **Purpose:** Turn an external DWG/DXF into an Onshape drawing.
- **Sources:** importing_a_drawing.htm, creating_a_drawing.htm, File/supported_file_formats.htm.
- **Selection inputs:** file (Documents page Import -> new document named after file; or in-document + Import -> new file tab + drawing tab named after file).
- **Parameters:** file; owner (Documents page).
- **Behavior / semantics:** translated to drawing format; model-space background color for imported DWG/DXF is a user preference. Supported import: DWG up to 2018, DXF up to 2013/2018, DWT 2013/2018. Imported drawings have no associative views (inferred). Image properties unavailable for embedded images in imported drawings.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** DXF/DWG library (DR-95).
- **Priority:** P2

### DR-91 Export drawing: dialog, PDF and raster/vector image formats
- **Purpose:** Produce PDF/SVG/PNG/JPEG (and CSV lists) from drawings.
- **Sources:** exporting_a_drawing.htm, File/exporting_files.htm, File/supported_file_formats.htm.
- **Selection inputs:** drawing tab (Export), or multiple drawings in Tab manager (Export drawings -> zip).
- **Parameters:**

| FS id | UI label | Type | Values / default | Visible when |
|---|---|---|---|---|
| - | File name | text | drawing name; export rules may template it | |
| - | Format | enum | PDF, DWG, DXF, DWT, SVG, PNG, JPEG, Inspection list (CSV), Drawing definition list (CSV) | |
| - | Version | enum | DWG/DXF: R11-14, 2000, 2004, 2007, 2010, 2013, 2018; DWT: 2013, 2018 | DWG/DXF/DWT. |
| - | Sheets | enum | all / current / Custom sheets (range or numbers, e.g. "1-3,5") | Exact option names besides "Custom sheets" not listed. |
| - | Overridden dimensions | enum | Show underlines, Hide underlines | |
| - | Text (PDF) | enum | Normal, Selectable | PDF. |
| - | Text (DWG/DXF/DWT) | enum | Export as text, Explode into polylines (notes), Explode all into polylines (annotations, notes, table text, dimensions) | |
| - | Color | enum | Color, Black and white, Grayscale | PDF. |
| - | Units | enum | drawing units or other unit | DWG/DXF. |
| - | Options | enum | Download, Download and store file in a new tab, Store file in a new tab, Email with file download link | |
| - | Apply export rules to contents | bool | | Multi-drawing zip. |
| - | Export splines as polylines | bool | | All but PDF. |
| - | Set z-height to zero and normals to positive | bool | | DWG/DXF/DWT. |
| - | Hide inspection items | bool | off (items shown) | |
| - | Exclude off-sheet content | bool | | DWG/DXF/DWT. |

- **Behavior / semantics:** asynchronous export with notification; output is a simplified drawing readable by most DWG readers; max custom sheet 1800 x 1800 in; placeholders "----" omitted by default. PDF "Normal" text = text drawn as vector outlines (inferred: "Normal" vs "Selectable"); B/W maps all colors to black; grayscale converts luminance. Per-sheet pages in one PDF (inferred). PNG/JPEG resolution options not documented. Mobile: DXF, DWG, DWT, PDF only.
- **Edge cases & errors:** `EXPORT_NOT_IMPLEMENTED`.
- **OCCT mapping:** none; vector PDF/SVG backends (DR-95); raster via 2D rasterizer.
- **Priority:** P0 (PDF) / P1 (SVG, PNG, JPEG)

### DR-92 Export drawing: DXF, DWG, DWT
- **Purpose:** CAD interchange of drawings.
- **Sources:** exporting_a_drawing.htm, custom_drawing_templates.htm, File/supported_file_formats.htm.
- **Selection inputs:** as DR-91.
- **Parameters:** see DR-91 (Version, Text, Units, Export splines as polylines, Set z-height to zero and normals to positive, Exclude off-sheet content, Sheets).
- **Behavior / semantics:** format layers BORDER_FRAME, BORDER_ZONES, TITLE_BLOCK (DR-06); strikethrough requires version 2013; Explode command on notes exports them as polylines; DWT export keeps only sheet 1/2 and non-view content (DR-05). Each sheet presumably as layout/paper space or separate file (not documented). Splines optionally approximated by polylines for old readers.
- **Edge cases & errors:** older versions may not support layers.
- **OCCT mapping:** none in OCCT open source; DXF writer library; DWG via ODA or LibreDWG (licensing in DR-95).
- **Priority:** P1 (DXF) / P2 (DWG, DWT)

### DR-93 Printing a drawing
- **Purpose:** Print-friendly output.
- **Sources:** printing_a_drawing.htm.
- **Selection inputs:** Document menu > Print...
- **Parameters:** OS/browser print controls.
- **Behavior / semantics:** opens print-friendly rendering; print or save. noshape: generate PDF (DR-91 pipeline) and hand to OS print dialog (inferred).
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-94 Drawing keyboard shortcuts
- **Purpose:** Default drawing hotkeys (customizable in preferences).
- **Sources:** drawings.htm, Home/keyboard_shortcuts_and_hotkeys.htm (Drawings), sheets.htm, drawing_sketch_tools.htm.
- **Selection inputs:** n/a.
- **Parameters:**

| Key | Action |
|---|---|
| f | Zoom to fit |
| w | Zoom window |
| Shift+z / z | Zoom in / Zoom out |
| d | Dimension (smart) |
| Shift+r | Radial dimension |
| Shift+d | Diameter dimension |
| Shift+h | Foreshorten dimension |
| Ctrl+m | Maximum/minimum dimension |
| Shift+q | Toggle midpoints and quad points during dimensioning |
| n | Note |
| Ctrl+q | Update drawing (all views and properties on each sheet) |
| l | Line |
| c | Center point circle |
| g | Corner rectangle |
| r | Center point rectangle |
| m | Trim |
| h / v / i | Horizontal / Vertical / Coincident constraint |
| Shift+l / b | Perpendicular / Parallel constraint |
| p | Projected view |
| s | Shortcut toolbar (Esc closes) |
| [ | Measure tool |
| Ctrl+s | Toggle Sheets panel |
| PgDn / PgUp / Home / End | Next / previous / first / last sheet |
| Delete | Delete selected |
| Shift+N | Rename selected sheet/view |
| Alt+LMB drag | Copy annotation/view |
| Shift (hold) | Disable inferencing during placement; move single member of group; keep all constraints visible |
| Ctrl+C / Ctrl+V | Copy / paste |

- **Behavior / semantics:** user-customizable; conflicts reported (Onshape-controlled, browser, override).
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### DR-95 Drawing engine architecture for noshape (design suggestion)
- **Purpose:** (design suggestion) Recommended implementation of the drawing subsystem on Rust + GPUI + OCCT. Not an Onshape feature; included for parity planning.
- **Sources:** derived from all items above; sectionpart.fs for section algorithm parity.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**

#### Hidden line removal (views)
- Per view: build `HLRAlgo_Projector` from the view `gp_Ax2` (orthographic; isometric = rotated axes; scale applied in 2D after projection to keep precision).
- Exact mode ("Best quality"): `HLRBRep_Algo` (Add shapes, `Update()`, `Hide()`), extract with a custom extractor modeled on `HLRBRep_HLRToShape::DrawEdge` that iterates `HLRBRep_Data` edges (`EDataArray`) and visible/hidden intervals (`HLRAlgo_EdgeIterator`), so every 2D segment keeps its source edge index (map back via the `TopTools_IndexedMapOfShape` used by `HLRBRep_ShapeToHLR`) and its class: sharp, smooth (Rg1Line -> tangent edges), sewn (RgNLine), outline/silhouette (with owning face), iso. Plain `HLRBRep_HLRToShape` compounds (VCompound, Rg1LineVCompound, OutLineVCompound, HCompound, ...) lose provenance and are only suitable for a first prototype.
- Fast mode ("Best performance", default): `BRepMesh_IncrementalMesh` + `HLRBRep_PolyAlgo` + `HLRBRep_PolyHLRToShape`; acceptable for large assemblies; re-run exact HLR for export when requested.
- Run HLR off the UI thread per view; cache results keyed by (model state hash, view params). Assemblies: one compound of located instance shapes; hidden parts removed from input; part-intersection edges via `BRepAlgoAPI_Section` on interfering pairs (prefilter `Bnd_Box`/`BRepExtrema_DistShapeShape`).
- Output to a 2D geometry store as `Geom2d` curves (lines, circles, ellipses, BSplines) projected onto the view plane (`HLRBRep` returns 2D-in-3D edges on the projection plane; convert with `BRep_Tool::Curve` + `GeomAPI::To2d`). Silhouettes of cylinders come back as lines; keep owning face for diameter inference.

#### Section, broken-out, crop, aligned cuts
- Mirror Onshape's `jogSectionCut`: bbox of targets in sketch-plane CS (+1% / 1e-5 m), project jog points, build the half-plane polygon on the "removed" side (respect partial-section shortening and offset), `BRepBuilderAPI_MakeFace` -> `BRepPrimAPI_MakePrism` through bbox depth -> `BRepAlgoAPI_Cut` per target (per instance for assemblies; skip excluded parts). Broken-out: spline face (`GeomAPI_Interpolate`, closed) extruded from depth point toward viewer, cut. Crop: `BRepAlgoAPI_Common` with full-depth prism (or 2D clip). Aligned: split into two segments, rotate revolved part with `BRepBuilderAPI_Transform`, then cut.
- Section faces for hatching: from `BRepAlgoAPI_Cut` history (`Modified`/`Generated` of tool faces) or classify result faces coplanar with cutting planes; hatch in 2D with `Geom2dHatch_Hatcher`; pattern library in AutoCAD .pat line-family format.
- Errors map to the `DRAWING_*_SECTION_CUT` messages (empty result, invalid, non-manifold via `BRepCheck_Analyzer`).
- Detail and break views are 2D operations on the parent's 2D result (clip with `Geom2dAPI_InterCurveCurve`, scale/translate with `gp_Trsf2d`).

#### Associativity
- Each 2D view entity stores provenance: source part/instance path + persistent topological id (noshape's naming system, same ids used by Part Studio features) + subtype (edge, silhouette-of-face, vertex, circle center, quadrant, thread line, COM, virtual sharp).
- Annotation references = (view id, provenance, point qualifier: start/end/mid/center/quadrant, 2D fallback coordinate). On update: regenerate view, resolve each reference by persistent id (and geometric matching fallback); unresolved -> dangling (red grip) per DR-43. Dimension values recomputed from 3D geometry where possible (true radius/length) and from 2D projected geometry for linear distances.
- View DAG (DR-24) with derivation parameters; version pinning per reference (DR-08/DR-11).

#### 2D annotation model and rendering
- Sheet-space entities (mm) on layers (Drawing, Title block, Border frame, Border zones), z-order, groups, attachments (to view entity, to other annotation, free). Types: dimension variants, note (rich text runs, fields), callout, datum, datum target, GD&T frame, surface finish, weld, hole callout, centerline/centermark/virtual sharp, sketch entities (+ 2D constraint solver), hatch, table, image.
- Style resolution: entity override -> drawing properties -> template defaults; every visual property addressable for Format painter and Styles panel.
- Text: TrueType via `ttf-parser`/`rustybuzz`; SHX stroke-font parser (custom) for AutoCAD fonts; GD&T/weld/surface-finish symbols as vector glyph library. Units/precision/tolerance formatting module (DR-13/DR-59/DR-60), fit tables from stdlib data.
- Display in GPUI: tessellate curves (`lyon` or GPUI path API) with linestyles (dash patterns scaled by pitch), zoom/pan, hit-testing via spatial index (R-tree) for snapping (endpoints, midpoints, centers, quadrants) and inferencing lines.

#### Export backends
- PDF: vector backend (Rust `pdf-writer`/`krilla` or `printpdf`), one page per sheet, true curves (Bezier conversion of arcs/splines), embedded fonts for "Selectable" text vs outlined glyphs for "Normal", color / black-and-white / grayscale transforms, embedded images. Also used for printing.
- SVG: `svg` crate or hand-written writer from the same display list; PNG/JPEG: rasterize display list with `tiny-skia`.
- DXF: OCCT open source has no DXF/DWG translator (only a commercial add-on). Options: Rust `dxf` crate (MIT; read/write, versions R12-2018) - preferred for licensing; `libdxfrw` (C++, GPL-2.0+; used by LibreCAD) - license incompatible with a non-GPL app. DWG/DWT: `LibreDWG` (GPL-3.0+, incompatible with proprietary distribution) or Open Design Alliance Drawings SDK (commercial membership). Recommend DXF first (P1), DWG via ODA if commercial, else defer (P2). Map layers BORDER_FRAME/BORDER_ZONES/TITLE_BLOCK, explode options, spline->polyline option, z=0/positive normals option.
- Import (DR-89/DR-90): same libraries; explode blocks/polylines, strip colors, convert simple text to notes.

- **Edge cases & errors:** large assemblies: use PolyAlgo + simplification (DR-27); HLR tolerance issues on tangent faces -> fall back to exact mode per view.
- **OCCT mapping:** HLRBRep_Algo, HLRBRep_PolyAlgo, HLRAlgo_Projector, HLRBRep_HLRToShape / custom extractor, BRepMesh_IncrementalMesh, BRepAlgoAPI_Cut / Common / Section, BRepPrimAPI_MakePrism, BRepPrimAPI_MakeHalfSpace, BRepBuilderAPI_Transform, GeomAPI_Interpolate, Geom2dHatch_Hatcher, Geom2dAPI_InterCurveCurve, GeomAPI::To2d, BRepGProp, BRepExtrema_DistShapeShape, BRepCheck_Analyzer. Gaps: no DXF/DWG in OCCT, no annotation/text layout, no PDF writer, HLR provenance requires custom extraction code.
- **Priority:** P0
