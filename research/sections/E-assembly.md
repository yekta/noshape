# E. Assemblies

Sources: Onshape Help `Assembly__*.md` (Oct 2026), `Home__keyboard_shortcuts_and_hotkeys.md`, `Home__triad_manipulator.md`, `View__isolate.md`, `View__make_transparent.md`, `bundle-command-actions.txt`, `bundle-i18n-strings.txt`, `fs-features.md` (Mate connector only; assembly mates are not FeatureScript features).
Tags: [DOCS] help pages, [BUNDLE] client JS bundle strings/IDs, [FS] FeatureScript std library. [INFERRED] = reasoned from sources/images alt-text but not stated verbatim. Nothing here was observed live.

Out of scope (one line): Simulation / Modal / Asynchronous simulation panels exist in Assemblies (right-side "Simulation panel" icon, Named positions can feed asynchronous simulation) - not documented here.

---

## 1. Assembly tab: concepts and basics

- An Assembly tab defines a hierarchical structure of part and subassembly instances, their degrees of freedom (mates) and relations. [DOCS]
- A document may have any number of Assembly tabs; new documents contain one empty Part Studio + one empty Assembly. [DOCS]
- Instanceable entities: parts, composite parts, surfaces, sketches, curves, meshes/mixed models, whole Part Studios (normal or rigid), assemblies (as subassemblies), standard content, non-geometric "items" (BOM only). [DOCS]
- Cross-document instances must come from a *version* (linked document). Same-document instances can reference the workspace (live updates). [DOCS]
- An assembly cannot insert another assembly as a version reference if they share the same element ID. [DOCS]
- Basic workflow [DOCS]: 1) Insert parts/assemblies, 2) Create mate connectors (optional), 3) Create mates, 4) Create relations (optional).
- Mates are solved *simultaneously* (order in list does not matter). [DOCS]
- Fix vs. Mate: Fix applies only in the assembly where applied; it does not carry over when the assembly is inserted elsewhere. Mating to the assembly Origin likewise does not carry into a parent assembly. Only one foundational component should be fixed or fastened to origin. [DOCS]
- Deleting an instance deletes all related mate connectors, mates, relations; exception: Groups are not deleted (member just removed; group deleted when last member deleted). [DOCS]
- Numeric fields in assemblies accept expressions, variables (`#var`) and trig functions. [DOCS]

### 1.1 Creating an Assembly tab
- `+` (Insert new tab) menu at bottom-left of tab bar > **Create Assembly**. [DOCS]

### 1.2 Assembly tab context menu (right-click the tab) [DOCS]
| Item | Behavior |
|---|---|
| Delete | Delete tab (even if active). Last remaining tab cannot be deleted. |
| Open in new browser tab | |
| Rename... | |
| Properties... | Metadata dialog; admin-set properties greyed out. Includes "Exclude from all BOMs" checkbox and "Subassembly BOM behavior". |
| Duplicate | Copy tab into same document; references maintained. |
| Copy to clipboard | Then `+` > Paste tab in another doc; source Part Studios/Assemblies are pasted along; no references to original kept. |
| Create Drawing of x... | New Drawing tab of entire assembly (solid bodies only). |
| Select as document thumbnail | |
| Move to document... | Move tab to a new document (created during op); links created if referenced. Prevented if it would leave a doc with no tabs. |
| Export | Export dialog. |
| Release / Release all configurations | Out of scope (PDM). |
| Create task | Out of scope. |

---

## 2. Assembly toolbar

General [DOCS]:
- Toolbar active when an Assembly tab is active. Toolbar is customizable: right-click toolbar > **Customize toolbar...** -> drag tool sets to reorder; drag a tool onto "New toolset" box to make a new set; drag a tool to the "Tools" box to remove; buttons **Save / Cancel / Reset to default** (reset applies per toolbar type: Part Studio, Assembly, Feature Studio). Grouped tools must be moved as an entire group; to reform a removed group use Reset to default.
- **S** opens the Assembly *shortcut toolbar* (mini toolbar) at the cursor. [DOCS][BUNDLE `OPEN_MINI_TOOLBAR`]
- **Alt/Opt+C** opens "Search tools" (command search) in top-right. [DOCS][BUNDLE `OPEN_COMMAND_SEARCH`]

### 2.1 Toolbar tools (left-to-right order)
Exact on-screen order is only shown as an image in docs; the order below follows the help-page tool listing + classic Onshape layout. [INFERRED order]. Command IDs are from [BUNDLE].

| # | Tool | Tooltip / label | Shortcut | Bundle action | Notes |
|---|---|---|---|---|---|
| 1 | Undo / Redo | Undo / Redo | Ctrl/Cmd+Z, Ctrl+Y / Shift+Cmd+Z | `UNDO_CHANGE`, `REDO_CHANGE` | Generic |
| 2 | Insert parts and assemblies | "Insert parts and assemblies" [BUNDLE] | **i** | `OPEN_INSERT_DIALOG` | Opens Insert dialog (sec 4) |
| 3 | Replicate | "Replicate" | - | `REPLICATE` | sec 11; dialog title action "Insert assembly replicate" [BUNDLE] |
| 4 | Linear pattern / Circular pattern | "Linear pattern", "Circular pattern" (dropdown pair) | - | `ASSEMBLY_LINEAR_PATTERN`, `ASSEMBLY_CIRCULAR_PATTERN` | sec 12 |
| 5 | Mirror | "Mirror" (Assembly mirror) | - | `ASSEMBLY_MIRROR_INSTANCE` | sec 13 |
| 6 | Mate connector | "Mate connector" | **Ctrl+M** | `CREATE_FEATURE_MATE_CONNECTOR` | sec 8 |
| 7 | Mate (types) | Fastened mate, Revolute mate, Slider mate, Planar mate, Cylindrical mate, Pin slot mate, Ball mate, Parallel mate | **m** opens Fastened mate dialog | `MATE` | One mate feature; type switchable in dialog via Mate type dropdown. Docs call Fastened/Revolute/Slider "first three mate features in the toolbar"; remaining types available in the same group/dropdown. sec 6-7 |
| 8 | Tangent mate | "Tangent mate" | - | `TANGENT_MATE` | Separate feature (no mate connectors) |
| 9 | Width mate | "Width mate" | - | `WIDTH_MATE` | Separate feature |
| 10 | Group | "Group" | - | `GROUP` | sec 10 |
| 11 | Relations | Gear relation, Rack and pinion relation, Screw relation, Linear relation | - | `MATE_RELATION` | One relation feature with a type selector (dialog: "confirm that Gear is selected"). sec 9 |
| 12 | Replace instance | "Replace instance" | - | `REPLACE_INSTANCE` | sec 14 |
| 13 | Display states | "Display states" | - | `DISPLAY_STATES` | sec 15 |
| 14 | Snap mode | "Snap mode" (toggle, highlighted when on) | **Shift+S** | `SNAP_MODE` | sec 20 |
| 15 | Show mates mode | "Show mates" mode (toggle) | **h** | `SHOW_MATES_MODE` | sec 21 |
| 16 | Create Part Studio in context | "Create Part Studio in context" | - | `CREATE_PART_STUDIO_IN_CONTEXT` | sec 18.2 |
| 17 | Update all references | "Update all references" (activates only when updates exist) | - | - | sec 19 |
| 18 | Measure / Mass properties / Section view / Interference detection | generic view/analysis tools | `[` (Measure), Shift+X (Section) | `SHOW_MEASURE_DIALOG`, `MASS_PROPERTIES`, `SECTION_VIEW`, `INTERFERENCE_DETECTION` | Shared with Part Studio |
| 19 | Simulation | - | - | - | Out of scope |

Non-toolbar but toolbar-like commands with shortcuts [DOCS/BUNDLE]: Hide/show mates **j** (`HIDE_SHOW_MATES`), Hide/show mate connectors **k** (`HIDE_SHOW_MATE_CONNECTOR`), Flip primary axis **a** (`FLIP_PRIMARY_AXIS`), Flip/reorient secondary axis (`FLIP_SECONDARY_AXIS`; **q** during snap drag), Lock mate inference **Shift** (hold), Make transparent **Shift+T**, Isolate **Shift+I**, Hide selected/hovered **y**, Show hidden **Shift+Y**, Fix (`FIX`), Copy/Paste occurrences Ctrl+C/Ctrl+V (`COPY_OCCURRENCES`, `PASTE_OCCURRENCES`), Rename selection Shift+N.

### 2.2 Right-side panel strip (icons on far right of graphics area, below View cube/View tools) [DOCS][BUNDLE tooltips]
Order as listed in docs/bundle tooltips [INFERRED order]:
- **Configuration panel** ("Configuration panel") - assembly configurations (see Variables/Configurations section).
- **Variable table** ("Variable table") - sec 22.
- **BOM Table** ("BOM Table") - sec 23.
- **Exploded views** ("Exploded views") - sec 16.
- **Named positions** ("Named positions") - sec 17.
- **Appearance panel**, **Custom tables**, **Simulation panel** (out of scope).
- Empty-state blurbs [BUNDLE]: "Exploded views help visualize how components fit together in an assembly." / "Add exploded views to visualize how components fit together in an assembly by moving instances from their mated positions." / "Named positions are multiple positional variations of an assembly, including its mate and subassembly positions." / "Add named positions to precisely locate parts and subassemblies. Create multiple positional variations of an assembly to capture mate positions and subassembly positions." / "Add parts and assemblies to build a bill of materials where you can view and edit metadata, as well as viewing instance counts and assembly structure." / "Add variables to drive mates and part configurations in the assembly." / "Add configurations to create multiple variants of an assembly using instance configurations, patterns, mates, mate connectors, or suppression states."

---

## 3. Assembly-relevant keyboard shortcuts [DOCS unless noted]

| Key | Action |
|---|---|
| i | Open Insert parts and assemblies dialog |
| m | Open (Fastened) Mate dialog |
| j | Show/hide all mates |
| k | Show/hide mate connectors |
| h | Toggle Show mates mode |
| Shift+S | Toggle Snap mode (in Part Studio Shift+S = Sketch) |
| Ctrl+M | Mate connector dialog |
| a | Flip primary axis (in mate/MC editing); in Snap drag and Standard content insert, flips orientation |
| q | During Snap drag: change alignment (secondary axis) |
| Shift (hold) | Lock mate inference (keep current face/edge locked while moving cursor) |
| Ctrl (tap) | Cycle through explicit mate connectors on the instance being inserted / dragged in Snap mode |
| Shift+Insert | Standard content: Insert furthest from selection |
| Alt+Insert | Standard content: Insert closest to selection |
| Enter | Commit dialog; Shift+Enter commits and reopens same command |
| Esc | Cancel command |
| Space | Clear selection |
| Delete/Backspace | Delete selection |
| Shift+N | Rename selected instance/feature |
| y / Shift+Y | Hide selected/hovered instances / show hidden instances |
| Shift+I / Shift+T | Isolate / Make transparent selected or hovered instances (hover + key repeatedly adds cumulatively) |
| Alt+click | Select through transparent |
| ` (backquote) | Select other (list of entities under cursor) |
| [ | Measure |
| Shift+X | Section view |
| Ctrl+C / Ctrl+V | Copy / paste instances (pasted where mouse is clicked; mates+MCs duplicated) |
| Up/Down arrows | With rollback bar selected in Exploded views panel: step through explode steps |
| Shift+/ | Keyboard shortcut map |

All shortcuts customizable in Preferences > Keyboard shortcuts (edit pencil, Disable shortcut, Reset to default, Reset all; conflict warnings). [DOCS]

---

## 4. Insert parts and assemblies dialog

Shortcut **i**. Dialog docked at left over the Instances list area [INFERRED]. [DOCS]

### 4.1 Layout / fields (top to bottom)
1. **Source tabs** (top): **Current document** | **Other documents** | **Standard content**. Onshape remembers the last selected source and reopens with it. [DOCS][BUNDLE exact labels]
2. (Other documents) Document browser with the same filters as the Documents page (Recently opened, Created by me, Shared, Public, teams, labels, etc.) + search box; pick a document; if doc has no version a notice offers to create one; if newer version exists a notice shows. Version graph icon ("Version graph" - insert from a specific version) and "Create version" icon (create version on the fly). [DOCS]
3. **Element-type tabs**: **Part Studios** | **Assemblies** (+ Variable Studios not applicable). [DOCS][BUNDLE]
4. **Filter icons** (Part Studios tab only): Sketches, Surfaces, Parts, Composite parts (+ Sheet metal flat patterns on mobile). Filters appear only for types that exist. Filtering parts/surfaces also shows meshes and mixed models. [DOCS]
5. **Insert entire Part Studio as rigid** toggle button [BUNDLE label] (Part Studios tab).
6. **Search** field with filter button at its left (common properties or all properties; multiple filters allowed). Searches name and properties incl. custom properties; partial words, multiple words, punctuation OK. [DOCS] Placeholder [BUNDLE]: "Search with filters or a partial name." / "Search by name, folder, or configuration".
7. **View released items** filter (if available): pick a particular revision; released items show a solid triangle icon. [DOCS]
8. **Result rows**: icon of entity type + name; under it: workspace/version icon, document name, workspace/version name, part number, release state. [DOCS]
9. **Configuration inputs** (when the selected Part Studio/Assembly is configured): dropdowns/checkboxes/values per input, **Generate** button to regenerate thumbnails/list for chosen config; **Set to last viewed configuration** dropdown icon with option **Reset to default**. Define configuration before clicking an item. [DOCS]
10. Part Studio entry shows default detail thumbnail of the whole Part Studio; individual parts/sketches/surfaces listed below it. [DOCS]
11. "Inserted" count updates with each insertion (mobile docs; desktop similar) [DOCS].
12. Checkmark (accept/close) and X. Status: "Select an item to insert." / "Select items to insert" [BUNDLE].

### 4.2 Insertion behavior [DOCS]
- Click a part -> inserts that part. Click the Part Studio thumbnail -> inserts all current parts (individually). With "Insert entire Part Studio as rigid" on, clicking the Part Studio inserts a single rigid Part Studio instance that auto-updates with Part Studio changes (new parts added automatically).
- Assemblies tab: click a subassembly -> inserted as subassembly instance; "Click an Assembly to add all of its subassemblies".
- Placement: **cursor inside the dialog** -> instance placed with its Part Studio origin aligned to the Assembly origin (same orientation/location as in Part Studio). Subsequent instances of the same part get a slight offset. **Cursor moved into graphics area** -> instance follows the cursor; left-click places it at the clicked location. Can click multiple times to insert multiple instances while dialog stays open [INFERRED from "insert multiple entities at a time"].
- Placed instances are free (not fixed, not mated).
- Pan/rotate allowed while inserting.
- Insertion cancelled message: "Assembly insertion has been cancelled." Undo hint: "Undo to remove instances" [BUNDLE].
- Tooltip "Insert and drag to snap" [BUNDLE] (snap-mode insertion).
- Mate connectors defined in the Part Studio come along; a part with explicit MCs has an expandable dropdown in the Instances list listing its MCs. Part-patterned MCs are all inserted if the part is the MC owner entity. [DOCS]
- Sketches inserted can carry MCs and be mated. [DOCS]

### 4.3 Snap-insert ("Assembling immediately") [DOCS]
- Requires Snap mode ON and the inserted component has explicit MCs.
- While dragging from the dialog, the component's source MC snaps to MCs (explicit or implicit-on-hover) of existing instances.
- Multiple explicit MCs: tap **Ctrl** to cycle which one is the source MC (highlighted).
- Snapping to a target and accepting the insert creates a **Fastened** mate between source and target MCs (then editable).
- No explicit MCs -> normal free drag even with Snap on.

### 4.4 Copy/paste from Part Studio [DOCS]
- Part Studio Parts list: right-click part > Copy Part (Ctrl+C); in Assembly right-click > Paste Part (Ctrl+V). Works for parts, surfaces, composite parts, same document.

### 4.5 Changing configuration after insert [DOCS]
- Instances list right-click > **Change configuration** -> inline config inputs -> checkmark / X. Hovering an instance shows a tooltip with the active configuration.

### 4.6 Editing a rigid Part Studio instance [DOCS][BUNDLE "Edit Part Studio instance"]
- Instances list right-click rigid Part Studio > **Edit** -> "Edit Part Studio instance" dialog: toggle include all parts / sketches / surfaces (three icons), Configuration dropdown. Checkmark to accept. Cancel message "Assembly instance editing has been cancelled." [BUNDLE]

### 4.7 Standard content tab [DOCS]
Supported libraries: ANSI (bolts & screws, nuts, washers), DIN (+ shaft keys), ISO, PEM (PennEngineering self-clinching). Instances show a Standard content icon.
Dialog fields in order:
1. **Standard** dropdown -> 2. **Category** -> 3. **Class** -> 4. **Component** (cascading).
5. **Size** dropdown with **Auto-size** icon: click icon then select a circular edge or cylindrical face; bolts/screws round *down* (no interference), nuts on a bolt shaft round *up*. Errors: "Unable to find matching size for selection. Selection may be too small or large for specific component", "Selection is not valid for auto-sizing. Selection must be a circular edge or cylindrical face or from existing standard content.", "To size the component, select a circular edge or cylindrical face." [DOCS][BUNDLE]
6. Further component-specific options (Length, thread type fine/coarse, material, etc.) - "tailored based on the selections".
7. **Part number** and **Description** (read-only with Edit pencil -> editable -> Save; Reset metadata icon) or (Pro/Enterprise) **Open metadata properties for this configuration** icon -> Properties dialog. Company-wide.
8. **Preview** thumbnail at bottom.
9. Buttons: **Insert**, **Insert closest to selection** (Alt+Insert), **Insert furthest from selection** (Shift+Insert); checkmark to finish.
Insert behavior:
- No preselection: Insert starts drag mode - fastener at cursor, hover hole to wake MCs, click to place; **a** flips orientation; creates Fastened mate.
- Preselected hole edges/cylindrical faces (any combination) -> fasteners placed at each.
- Preselected planar face -> fastened to *all* holes on that face regardless of size fit.
- Bolt has one mate reference; washers/nuts have two -> stackable; washer inserted into a hole with a mated bolt is placed under the bolt head. Closest = against bolt head / top of selected washer; Furthest = against part / bottom.
- Hole feature holes pass fine/coarse thread to fastener; otherwise coarse.
- Messages: "To insert closest to selection, preselect planar faces containing holes first." / "...furthest..." [BUNDLE]
Editing: right-click instance > **Edit standard content instance** -> same dialog, Standard/Category/Class/Component locked; change Size/Length/etc.; **Update** under preview; checkmark. Also **Select instances with the same configuration** / **Select instances with the same part and same configuration** to batch-edit. Standard content cells are greyed in BOM. [DOCS]

---
## 5. Instances list

The left panel in an Assembly tab ("Instances and mates" [BUNDLE tooltip]); collapse/expand with the Instances-and-mates icon on its right edge. [DOCS]

### 5.1 Structure (top to bottom) [DOCS]
1. **Configuration section** (if assembly is configured): config input controls at top of list.
2. **Header row**: top-level Assembly name (click selects the entire assembly); **Filter** box with filter-icon dropdown; **New folder** icon. Instance count label "Instances (__instanceCount__)" [BUNDLE].
3. **Instances**: parts, Part Studio (rigid) instances, subassemblies (expandable, contain their own instances + their own Mate features folder), patterns, replicates, mirrors (expand to their child instances), groups-as-inserted, folders, standard content, **Items** (non-geometric BOM items, listed in an Items list).
4. **Mate features** section (header "Mate features" [BUNDLE]): explicit Mate connectors, Mates, Groups, Relations, Replicate/Pattern/Mirror features' generated mates, mate folders. Listed chronologically by default; drag to reorder (Ctrl+click multi). Implicit MCs listed as children under the mate that first uses them; an MC used in several mates is listed once.
- Instance naming: `Name <n>` (e.g. `Housing <1>`, `Housing <2>`); part renames in Part Studio propagate. New subassemblies named `Assembly X <x>`.
- Optional right-side columns: part number, revision, state ("Show instance list properties" context command, or Preferences > Assembly settings > Display instance list properties).
- Hover a mate/MC -> read-only tooltip with its offset & limit definitions; double-click -> edit. Hover a mate -> associated instances highlighted and its glyph shown. [DOCS]
- Selecting in graphics auto-scrolls + highlights row; if collapsed, graphics context menu has **Go to item in list** (absent if already visible). [DOCS]
- Eye icon on hover per row toggles visibility. [DOCS]

### 5.2 Filter box [DOCS][BUNDLE]
Text filters names. Colon commands (also in filter dropdown); combinable (e.g. `:mate :errors`, `:part :hidden name`):
| Command | Filters | Dropdown label [BUNDLE] |
|---|---|---|
| :part | parts (+optional name) | |
| :assembly | subassemblies | "Filter by subassemblies" |
| :mate | mates | "Filter by mates and relations" |
| :matelimits | mates with >=1 limit | "Filter by mates with limits" |
| :item | non-geometric items | |
| :folder | folders | |
| :errors | errors | |
| :warnings | warnings | |
| :suppressed | suppressed | |
| :hidden | hidden | |
| :shown | shown | |
Also "Filter by mate connectors" [BUNDLE].

### 5.3 Icons / status indicators [DOCS][BUNDLE tooltips]
| Icon | Meaning | Tooltip text [BUNDLE] |
|---|---|---|
| Assembly icon | assembly/subassembly | |
| Instance icon | part instance | |
| Imported (derived) instance icon | imported | |
| Configured part / configured assembly icons | configured | |
| Standard content icon | standard content | |
| Fastened-to-origin icon (on assembly) | assembly fastened to origin | "Assembly is fastened to origin" |
| Fix icon (pin) right of instance | instance fixed | "Part is fixed" |
| "Unfix" icon on subassembly | subassembly contains a fixed instance | "Part(s) inside are fixed" |
| Degrees-of-freedom (triad) icon | not fixed, has DOF (top-level only; not shown for instances inside non-rigid subassemblies) | "Has degrees of freedom" |
| Rigid assembly icon (replaces assembly icon) | zero DOF + one fixed part => auto-rigid | "__partName__ (Rigid)" |
| Configured rigid assembly icon | | |
| Lock icon right of subassembly | locked to current position / following named position; hover shows unlock icon | "Locked at current position", "Locked at named position __positionName__", "Locked at subassembly tab position" |
| Mate limit indicator (right of mate) | mate has limits | "__mateName__ has limits" |
| Linked/version icons, revision triangle, pin thumbtack, blue background = update available | see sec 19 | "Versioned __instanceType__ from other document with newer version", etc. |
| In-context solid arrow / active in-context / dashed arrow | primary instance of context / active context / previous primary | "Instance is not primary instance of context." |
| Context update blue dot | context update available | "Update available for __contexts__" |
| Tessellation icon | best tessellation forced | "Highest quality tessellation has been forced..." |
| Red error icon (+ expand arrow to locate) | error; hover for message | "__partName__ has error: __regenError__", "Assembly contains mate error", "Subassembly contains mate error", "Folder contains mate error", "Assembly contains instance error", "Instance contains error" |
| Warning | | "__partName__ has warning: Fastened to origin even though Assembly has fixed part(s)", "More than one part is fixed", "More than one part inside is fixed", "Other part(s) fastened to origin", "Contains both fixed part(s) and part(s) fastened to origin" |
| Dynamic suppression expression icon | suppressed by expression | "__partName__ (Suppressed with expression)" |
| Strikethrough/grey name | suppressed | "__partName__ (Suppressed)" |
| Dimmed | hidden | "__partName__ (Hidden)" |
| Inactive | | "(Inactive: One or more mates is inactive or suppressed)", "(Inactive: All instances are suppressed)", "(Inactive: One or more instances is suppressed)" |
| Folder count `(n)` | items in folder | |

### 5.4 Instance context menus [DOCS]
Part instance / subassembly instance / top-level assembly menus (items vary by context; listed in doc order):
- **Properties** (part/assembly properties dialog)
- **Instance properties** (instance-level, e.g. Reference designator)
- **Edit** (rigid Part Studio instance only)
- **Show/Hide instance list properties** (assembly menu)
- **Show/Hide center of mass**
- **Rename**
- **Unfix all** (top-level/subassembly: unfixes all fixed instances within subassemblies, folders, parametric Part Studios, patterns, replicates)
- **Add exploded view** (assembly menu)
- **Release** (out of scope)
- **Hide / Show**, **Hide other instances**, **Hide all instances**
- **Isolate**, **Make transparent** (dialogs, see 24.6)
- **Suppress / Unsuppress** (suppressing an instance makes all its mates inactive; suppressing a seed makes its patterns/replicates inactive)
- **Dynamic suppression** > Add expression / Edit dynamic suppression > Edit expression / Remove expression
- **Fix / Unfix**
- **Use best available tessellation / Use automatic tessellation setting**
- **Check interference**
- **Replace instances** (labels [BUNDLE]: "Replace __name__…", "Replace __numberBiggerThanOne__ instances…")
- **Edit in context** (submenu of existing contexts + "Create new context" if any exist)
- **Update context** > context list; **<context name> > Set as primary instance**
- **Add mate connector to instance origin** (opens MC dialog prefilled; lands in Mate features)
- **Switch to [Part Studio]** / Switch to Assembly
- **Copy [Part]**
- **Add to named positions** (subassembly)
- **Lock/follow position to** > Lock current position | <named positions...> (checkmark on active)
- **Change configuration**
- **Move to new subassembly**, **Dissolve subassembly**, **Create empty subassembly**
- **Change to version** (Reference manager); **Pin reference** (on link icon)
- **Edit standard content instance**, **Select instances with the same configuration**, **Select instances with the same part and same configuration** (standard content)
- **Show mates** (graphics context menu)
- **Go to item in list** (graphics context menu)
- **Export**, **Where used**, **Revision history**, **Create task**, **Add comment** (out of scope details)
- **Zoom to selection**
- **Create drawing of [Part]**
- **Expand** > Expand selected / Expand all; **Collapse** > Collapse selected / Collapse all (Expand all/Collapse all only when no subassembly selected)
- **Add selection to folder**
- **Delete**
- Graphics-area general menu also: Show all, Show all parts, Create selection, Zoom to fit, Isometric, Clear selection.

### 5.5 Restructuring [DOCS]
- Drag-and-drop instances: reorder list (drives BOM item order); drop onto a subassembly to move into it (subassembly tab is modified); drag out to top level. Mates maintained. Fails if it would modify a linked (versioned) document. Error [BUNDLE]: "Failed to move instance from subassembly to another instance of the same subassembly.", "Cannot move pattern into the assembly tab because it would create cyclical references".
- **Move to new subassembly**: select parts/patterns > dialog "Move to new subassembly" lists dependencies also moved > **Move**. Creates new Assembly tab + subassembly instance with its own (possibly empty) Mate features.
- **Create empty subassembly** (aka Create new subassembly): new empty Assembly tab + instance `Assembly X <x>`; whatever is built in that tab flows into the instance.
- **Dissolve subassembly** (one or many; must be in parent tab): moves instances + mates up to parent; subassembly tab left empty (not deleted). Not allowed if subassembly contains folders. Versions referencing it stay immutable.
- **Lock/follow position to**: Lock current position (freezes subassembly mates -> performance) or follow a named position of the subassembly (regenerates then locks). Lock icon shown; right-click lock icon to unlock; hovering a subassembly shows unlock icon - right-click it to lock quickly. Messages [BUNDLE]: "Lock __itemName__ at current position", "Lock to named position", "Assembly is unlocked from named position.", "__assemblyName__ locked at named position __positionName__.", "Mates with errors cannot be locked."

### 5.6 Folders [DOCS]
- Instances folders: New folder icon (folder at bottom; or around current selection placed at first selection's location) or right-click > Add selection to folder -> "Folder name" dialog. Nested folders allowed. Drag in/out; count `(n)` updates. Desktop only for creation.
- Instances-folder context menu: Rename; Hide/Show (submenu Hide other instances, Hide/Show all instances); Isolate; Make transparent; Suppress; Dynamic suppression; Add selection to folder; Expand; Collapse; Delete (deletes folder **and contents**); Unpack folder (moves contents to root, deletes folder). Eye icon on folder hides/shows all.
- Mate features folders: select mates/MCs > New folder or Add selection to folder. Cannot leave the Mate features list. Context menu: Rename; Hide/Show (Hide other mates, Hide/Show all mates); Isolate; Make transparent; Suppress (all mates/MCs inactive); Dynamic suppression; Add selection to folder; Expand; Collapse; Delete (deletes contained mates); Unpack folder.

### 5.7 Dynamic suppression [DOCS]
- Applies to parts, Part Studios, subassemblies, items, mate connectors, mates, folders. Right-click > Dynamic suppression > Add expression -> dialog with expression (e.g. `#depth >= 0.5 in`) -> checkmark. Expression icon shown; click icon to edit. Suppressed rows appear struck through. Strings [BUNDLE]: "Dynamic suppression added/updated/removed.", "Dynamic suppression: {__expression__}".

---

## 6. Mates: common model and dialog

### 6.1 Concepts [DOCS]
- Exactly one mate typically between two instances; mate type encodes remaining DOF. Mates align a mate connector (local CS) on instance A with one on instance B.
- Mate connectors are explicit (created by MC tool, in Part Studio or Assembly; top-level in list; selectable from list during mate creation) or implicit (created on the fly during mate creation; listed under the mate; cannot be picked from list).
- Mate targets: parts, subassemblies, sketches, surfaces, the assembly **Origin**.
- First selected MC's coordinate system defines the mate frame; offsets are relative to the first MC; displayed distance values measured *from the second MC to the first* along the mate CS.
- Mates solved simultaneously. While a mate dialog is open, the solver does not continuously re-solve all mates; only the instances in the current mate move; press **Solve** to solve everything.
- Mate DOF names (FS enum `MateDOFType`) [FS]: Tx "Translational X", Ty "Translational Y", Tz "Translational Z", Rz "Rotational Z", Ryp "Rotational Y'", Rzp "Rotational Z'" (ball/parallel compound rotations).

### 6.2 Mate dialog layout (Fastened..Parallel) [DOCS + INFERRED order]
1. **Mate type** dropdown/selector (Fastened, Revolute, Slider, Planar, Cylindrical, Pin slot, Ball, Parallel) - change type of an existing mate without redefining connectors. [DOCS]
2. **Mate connectors** list field (2 picks): first = top item, second = bottom item.
   - Each picked item shows a **Mate connector icon**: click to open the Mate connector sub-dialog to edit origin type/entity, alignment, location, primary/secondary axis (only for MCs local to this assembly; Part Studio/subassembly/linked MCs must be edited at source).
   - **Reorder items** link -> drag handles on the right of each row to swap order -> **Done**. Order matters (offset frame, which MC flips, Pin slot slot/pin).
   - Implicit MC selection mode icon: "Select mate connector (__keybind__)" / "Select mate connectors (__keybind__)" [BUNDLE].
3. **Flip primary axis** (arrow icon; key **a**) - flips Z of the *top* MC; if the top one is constrained, the bottom one flips instead.
4. **Reorient secondary axis** icon - rotates XY quadrant 90 deg per click (same top/bottom rule).
5. **Animate** play button - previews unlimited motion of this mate's DOFs, ignoring other mates (with limits if set, per Limits section).
6. **Offset** checkbox (types: Fastened X/Y/Z; Slider X/Y; Revolute Z; Pin slot Z; Planar Z; not Cylindrical/Ball/Parallel) [DOCS]
   - Offset distance fields per allowed axis (label pattern "X"/"Y"/"Z" offset) - signed values; negative reverses.
   - **Rotation**: axis selector (Rotate about X/Y/Z - same enum as MC `RotationType` [INFERRED]) + rotation angle; available for Slider, Revolute, Pin slot, Planar, Fastened.
7. **Limits** checkbox (not for Fastened, Tangent, Width) -> per-DOF **min / max** fields (each optional; blank = unbounded that side). Labels [BUNDLE]: "Min X distance limit", "Max X distance limit", "Min Y distance limit", "Max Y distance limit", "Min Z distance limit", "Max Z distance limit", "Min Z angle limit", "Max Z angle limit", ball "cone angle". Validation: "Upper limit should be greater" [BUNDLE].
8. **Solve** button - solve all mates incl. this one.
9. Accept checkmark / Cancel X (dialog header).

Heads-up display: after both MCs are selected, a small HUD appears at the cursor with a checkmark - tooltip "Commit current mate and start a new mate" (dialog stays open for next mate); dialog's own check = "Commit new mate" [DOCS][BUNDLE].

Mate value visualization [DOCS]:
- When a mate is selected/being edited: X/Y/Z axis graphics of mate frame, offset (dashed line + value + axis label), limits (dashed line with bars at ends; dashed = direction/distance of motion, solid = limit).
- While dragging, live DOF distance/angle values shown in graphics; double-click a value to type a new one -> moves instance there (not persisted; for estimation/measurement).
- Mate values settable for all mates except Ball, Fastened, Tangent, Width.

### 6.3 Mate connector inference during mate creation [DOCS]
- Hovering faces/edges/vertices shows implicit MC inference points (dots); point closest to cursor shows a full triad preview.
- Points: face centroid, edge midpoints, vertices/corners, arc/circle centers, centroids of regions within a planar face (holes, slots, "negative space" of cuts), cylinder axis top/middle/bottom, cone virtual sharp, curve end/control/midpoints.
- Circular faces/edges have only a centroid -> fastest picks.
- Hold **Shift** to lock the current reference face/edge (other inferences suppressed) while moving cursor -> MC snaps to nearest point on that entity. Bundle labels: "Lock mate inference", "Suppress inferencing".
- Hovered connected entities cross-highlight.
- On second pick, instance B moves so MCs coincide with Z axes aligned (instances "snap" into place).

### 6.4 Mate indicators (glyphs) [DOCS]
- Each mate has a glyph in graphics; shape hints motion type (fastened, revolute, slider, planar, cylindrical, pin slot w/ arrow along slot, ball, parallel, tangent, width, group).
- Colors: **blue/white = OK**, **grey = suppressed/inactive**, **red = problem**.
- Mates hidden by default in graphics; **j** toggles all; eye icon per mate; Show mates mode (**h**) shows on hover.
- Selecting a glyph/MC/relation in graphics highlights its mate feature + instances in list. Right-click glyph -> mate context menu (Edit, Animate, ...). Bundle label "Mate indicator".

### 6.5 Mate context menu [DOCS]
Rename; **Edit...** (works even when suppressed; double-click also edits); **Apply limit position** (submenu min/max per DOF; messages "Moved mate to min/max X|Y|Z limit", "Moved mate to max cone angle", "Unable to apply limit", "Mates with errors cannot be applied."); **Reset** (return to start/home position); **Animate** ("Animate __mateName__…"); Hide / Show; **Show all mates**; **Isolate** (Exit isolate shown when active); **Make transparent** (slider; expand by distance or mate connectivity); **Suppress / Unsuppress** (suppressed mates still editable; mates auto-inactive when an owner instance is suppressed); Dynamic suppression; Add selection to folder; **Edit named position driver mate** (sec 17); Configure (assembly configurations); Clear selection; Delete.
- MC context menu: Hide, Hide other Mate connectors, Show, Show all Mate connectors, Edit, Delete, etc.

### 6.6 Animate mate dialog [DOCS][BUNDLE "Animate mate"]
- Open via mate (or mate indicator) context menu > Animate. Drives assembly from one DOF; all other mates/relations enforced (unlike the in-dialog preview).
- Fields: DOF selector if mate has >1 DOF ("Select degree of freedom"); **Start value** (default = min limit if set); **End value** (default = max limit); up to 36000 deg (100 rev); **Steps** (linear interpolation, inclusive, min 2; ~60 steps/s playback; more steps = slower); **Playback type**: Single | Reciprocate | Loop; **Reverse** arrow icon; **Current value** (read-only, updates live, stays at stop point); **Play** / **Stop**.
- Not recommended as driver: Fastened, Tangent, Ball. Works with Isolate, mate indicators, MCs visible.

### 6.7 Other mate operations [DOCS]
- Reorder mates by drag in Mate features (cosmetic).
- Copy/paste mated instances: select > Copy items > Paste items (pasted at click point); MCs and mates duplicated.
- Apply named positions, configurations can drive mate offsets/limits/suppression [BUNDLE "Choose instances and mate parameters to be driven by this configuration input"].
- Mate solve performance: "Mate solve time : __solveTimeInSeconds__", "<strong>__elementName__</strong> took __mateSolveTime__ to solve mates." [BUNDLE]

---

## 7. Mate types reference

### 7.1 Summary table [DOCS]
| Mate | DOF remaining | Free DOFs | Offset axes | Rotation offset | Limits | Mate values | Notes |
|---|---|---|---|---|---|---|---|
| Fastened | 0 | - | X, Y, Z | yes | no | no | 2 MCs |
| Revolute | 1 | Rz | Z | yes | Z angle min/max | yes | 1st MC rotates, 2nd stationary |
| Slider | 1 | Tz | X, Y | yes | Z distance min/max | yes | 1st MC slides, 2nd stationary |
| Planar | 3 | Tx, Ty, Rz | Z | yes | X, Y distance; Z angle [INFERRED set] | yes | |
| Cylindrical | 2 | Tz, Rz | - | - | Z distance, Z angle | yes | |
| Pin slot | 2 | Rz, Tx | Z | yes | X distance, Z angle | yes | slot along X of slot MC |
| Ball | 3 | Rx, Ry, Rz | - | - | max cone angle from Z | no | select ball center only |
| Parallel | 4 | Tx, Ty, Tz, Rz | - | - | distances for relative displacement (Z distance) [INFERRED] | yes | keeps Z axes parallel |
| Tangent | varies | - | - | - | - | no | faces/edges/vertices, no MCs |
| Width | 2 | translation along + rotation about center plane normal | - | - | - | no | tabs centered between width MCs |

### 7.2 Fastened [DOCS]
- Removes all 6 DOF. Dialog: Mate type, Mate connectors (2), flip/reorient, Offset (X, Y, Z any combination + rotation axis + angle), Solve. No limits.
- Click instance after: manipulator shows no motion allowed.
- Typical: bolt in hole, welded parts. Default mate for **m** shortcut and snap insertion.

### 7.3 Revolute [DOCS]
- Rz only. First MC = rotating point; second = stationary.
- Limits: Z angle min/max. Offset: Z only + rotation.

### 7.4 Slider [DOCS]
- Tz only. First MC = sliding point, second = stationary.
- Limits: Z distance min/max (negative values if in wrong direction). Offset: X and Y (any combination) + rotation.

### 7.5 Planar [DOCS]
- Tx, Ty, Rz (3 DOF). First MC = moving, second = stationary. Offset: Z only + rotation. Limits checkbox available (per-DOF fields).
- Often paired with Tangent (pin in curved slot).

### 7.6 Cylindrical [DOCS]
- Tz + Rz. Limits for both Z distance and Z angle. No offset.

### 7.7 Pin slot [DOCS]
- Rz + Tx. Docs conflict on order: steps say "select pin's MC first" (first = pin + rotation; second = slot + translation), transcript says slot first. Implementer: field order matters; use Reorder items to fix. Slot direction = X axis of slot MC; glyph has arrow along slot.
- Limits: X distance (e.g., +/- half slot length) and Z angle (0/0 to prevent rotation). Offset: Z only + rotation.

### 7.8 Ball [DOCS]
- Rx, Ry, Rz. Select center of socket and center of ball (only ball center selectable). First MC rotates, second stationary.
- Limits: single field = maximum rotational angle from the Z axis (cone). Manipulator drag cannot pass the limit and the limit is drawn.

### 7.9 Parallel [DOCS]
- 4 DOF: Tx, Ty, Tz + Rz; keeps MC Z axes parallel. Limits: min/max for relative displacement. No offset.

### 7.10 Tangent [DOCS]
- No MCs. Fields: first entity (face/edge/vertex of instance 1), second entity (face/edge/vertex of instance 2), **Tangent propagation** checkbox (default checked - also constrains adjacent tangent-connected faces), **Flip primary axis** (works only when two faces selected).
- Restrictions: no offset surfaces; only swept faces (planes, cylinders, cones, tori...), not splines; propagation no effect with two vertices; cannot be used with Relations; named positions may not hold.
- Bundle label "Tangent mates".

### 7.11 Width [DOCS]
- Fields: **Tab mate connector(s)** (1-2 MCs; may be from same or different instances) and **Width mate connectors** (exactly 2; may be same or different instances). Tab and Width selections must not share instances.
- Behavior: tab MC(s) kept symmetric about the center plane between the two Width MCs; tab instance translates along / rotates about center plane. Two tab MCs on different instances remain mutually symmetric (moving one moves the other).
- Implicit and explicit MCs allowed; two MCs on the same part allowed (unlike other mates). Cannot be used with relations. "Show Width mate" from Mate features list displays it.

---
## 8. Mate connector (Assembly)

Shortcut **Ctrl+M**; hide/show all **k**. Available in Part Studio and Assembly. [DOCS]

### 8.1 Dialog fields (order per [FS] `mateConnector`; assembly dialog uses the same feature UI minus owner/attachment fields [DOCS])
| # | Field | Type / options | Default | Visibility |
|---|---|---|---|---|
| 1 | **Origin type** | On entity / Between entities | On entity | always |
| 2 | **Origin entity** | 1 pick: edge, vertex, non-construction face, mesh, existing MC, Origin (graphics or list), inference point | - | always |
| 3 | **Between entity** | 1 face | - | Origin type = Between entities (MC placed halfway between the two) |
| 4 | **Realign** | checkbox | off | always |
| 4a | Primary axis entity | face or edge -> controls Z | - | Realign on |
| 4b | Secondary axis entity | face or edge -> controls X/Y | - | Realign on |
| 5 | **Move** | checkbox | off | always |
| 5a | X translation / Y translation / Z translation | length | 0 | Move on |
| 5b | Rotation axis | Rotate about X / Rotate about Y / Rotate about Z | Rotate about Z | Move on |
| 5c | Rotation angle | angle | 0 deg | Move on |
| 6 | Owner entity / Select owner entity / Attachment | Part Studio only | | Part Studio only |
| 7 | **Flip primary axis** | arrow toggle (first in row) | off | always |
| 8 | **Reorient secondary axis** | cycles +X, +Y, -X, -Y | +X | always |
- Hidden param `entityInferenceType` [FS]: PART_ORIGIN, POINT, CENTROID, CENTER, MID_POINT, TOP_AXIS_POINT, MID_AXIS_POINT, BOTTOM_AXIS_POINT, ORIGIN_X/Y/Z, LOOP_CENTER, VIRTUAL_SHARP - records which inference point was clicked.
- Errors [BUNDLE]: "Could not find selected mate connector.", "Error getting transformation for selected mate connector.", "Cannot create Part Studio from invalid mate connector."

### 8.2 Inference points and default orientation [DOCS]
| Hovered entity | Points | Z orientation |
|---|---|---|
| Planar face | every vertex, arc center, edge midpoint, face centroid, centroids of contained regions (holes/slots) | normal to face |
| Cylindrical (incl. partial) face | on axis at middle and both ends | along axis |
| Linear edge / sketch line | middle and ends | along line |
| Circular edge / sketch circle | center (centroid of axis) | along circle axis |
| Conical face | top, middle, bottom, virtual sharp | along axis |
| Curve endpoint / control point | - | global Z |
| Curve | closest end/control/mid point | tangent to curve |
| Vertex | - | normal to given face |
- Clicking anywhere on a face (not on a dot) places MC at centroid. Before snap highlight, a centroid point icon may show. Shift locks inference. Cross-highlight of connected entities on hover.
- MC can be placed on an existing MC then realigned/moved.

### 8.3 Visibility / list [DOCS]
- Explicit MCs at top level of Mate features; context menu Hide, Hide other Mate connectors, Show, Show all Mate connectors; eye icon. Visibility independent between Part Studio and Assembly.
- Part Studio MCs advantages: can reference sketch entities & origin from list; reusable on every instance.
- Editing from a mate dialog only for MCs in current assembly.
- Implicit MCs in other tools: Patterns/Mirror axis fields have a **Select mate connector** icon to pick implicit MCs; then MC icon in field to edit.

---

## 9. Relations

Toolbar: Relations (single `MATE_RELATION` command, dialog shows relation-type selector) [DOCS][BUNDLE]. Relations constrain DOFs of existing mates; also selectable from subassembly mate lists.
General flow [DOCS]: click relation tool -> confirm type -> select mates in graphics or Mate features list -> if a mate has exactly the needed DOF, the DOF is shown with an icon (e.g. Tz for Slider, Rz for Revolute); if it has more DOFs, a second popup "Select degree of freedom" asks which -> enter ratio/distance -> optional Reverse -> accept. Once a DOF is chosen it cannot be changed without removing that mate from the field, changing the mate type, or deleting the mate.

| Relation | Inputs (order) | Valid mates | Value field | Other |
|---|---|---|---|---|
| **Gear** | Mate 1 (rotational DOF), Mate 2 (rotational DOF) | Revolute (exact), Cylindrical, Planar, Pin slot, Parallel (need extra mates to hold position) | **Ratio** (e.g. driving teeth / driven teeth, 20/40 = 0.5) | **Reverse direction** checkbox |
| **Rack and pinion** | Mate 1 rotational (pinion), Mate 2 linear (rack) | Revolute + Slider exact; others prompt DOF | **Distance per revolution** (linear travel per pinion rev; units selectable) | **Reverse** |
| **Screw** | single Cylindrical mate (rotation + translation of same mate) | Cylindrical required | **Distance per revolution** | **Reverse** |
| **Linear** | Mate 1 linear, Mate 2 linear | Slider exact; others prompt DOF | **Ratio** (default 1; default motion opposite direction) | **Reverse** (same direction) |
- Tangent and Width mates cannot be used with relations. [DOCS]
- Relation glyphs shown in graphics; selecting highlights the feature. [DOCS]

---

## 10. Group [DOCS]

- Fixes relative position of selected instances (by their origins; ignores geometry changes) without mates; best for parts from the same Part Studio or purchased parts.
- Dialog: single **Instances** selection field (preselection supported; pick from list, graphics, or box-select). Accept -> Group feature added to Mate features list with child entries.
- Group moves as one rigid unit; still needs Fix or a mate to be grounded. Unlimited groups.
- Rules: a part inside a subassembly cannot be picked individually (selection promotes to the subassembly); subassembly must be rigid to be grouped; replicating groups unsupported.
- Suppress group -> reposition members -> unsuppress = group captures new relative positions (Undo to revert). Part Studio position changes update group.
- Context menu: Hide instances / Show instances, Edit, Suppress, Delete, etc.
- Grouped assembly when inserted elsewhere shows as one entity.
- Group glyph has OK/problem/suppressed states.

---

## 11. Replicate [DOCS]

- Takes a mated seed (e.g., bolt) and creates copies mated to all matching geometry.
- Dialog fields in order:
  1. **Instances** - seed instance(s) (do not group them).
  2. **Mate to be matched** - auto-filled if seed has a single mate; otherwise pick the mate from Mate features.
  3. **Match scope** (options depend on whether the seed mate referenced a face or an edge): Face: *Match faces in parts*, *Match faces in features*, *Match individual faces*; Edge: *Match edges on plane*, *Match edges on face*, *Match individual edges*.
  4. **Parts to find match in** (or features / faces / plane / edges per scope).
- Result: "Replicate" feature in Instances list with generated instances and its child mate features. Context menu **Dissolve Replicate** -> individual mates, feature removed.
- Seed may have only one external mate. Suppressing seed makes replicated instances inactive. Errors: hover orange dialog title for hints. Bundle: "Successfully replicated".

---

## 12. Assembly Linear / Circular Pattern [DOCS]

### 12.1 Linear pattern fields
1. **Instances** (parts, subassemblies, sketches, surfaces; graphics or list).
2. **Direction** (edge or face; or MC via Select mate connector icon; MC icon in field to edit MC).
3. **Distance** (spacing).
4. **Instance count** (min 1).
5. **Opposite direction** toggle.
6. **Equal spacing** checkbox (instances spread within Distance total).
7. **Centered** checkbox (seed in center; N each way inclusive of seed).
8. **Second direction** checkbox -> Direction, Distance, Instance count, Opposite, Equal spacing, Centered for dir 2; **Third direction** likewise.

### 12.2 Circular pattern fields
1. **Instances**. 2. **Axis of pattern** (edge, face, conic/cylindrical face, implicit/explicit MC). 3. **Angle** (between instances). 4. **Instance count** (min 1). 5. Opposite direction. 6. **Equal spacing**.

Notes: Patterned instances join seed's group; patterned subassemblies remain non-rigid (moving one updates others); suppressing seed makes pattern inactive; patterns movable to subassemblies. Title action "Insert assembly pattern" [BUNDLE].

---

## 13. Assembly Mirror [DOCS]

Dialog fields:
1. **Instances to mirror** (parts, surfaces, subassemblies; list selection recommended). Prompt [BUNDLE] "Select entities to be mirrored."
2. **Mirror plane** - hover plane/edge/planar face to pick implicit MC (MC selection icon on by default; toggle off to select a face); or explicit MC; MC icon edits it.
3. **Mirror strategy** field -> opens **Mirror strategy** table ("Select how instances are mirrored." [BUNDLE]) with columns:
   - **Instance** (read-only), **Source** (read-only version), **Symmetry** (read-only symmetric/asymmetric),
   - **Mirror action**: *Transform* (copy + transform in current assembly; default) / *Derived* (new mirrored part via Derived feature in a Part Studio inside a "Mirrored" folder; name suffix `-Mirrored`),
   - **Mirror reference** (link opens tab; info icon if missing),
   - **Placement type**: Auto position, Bounding box XY, Bounding box YZ, Bounding box ZX, Origin XY, Origin YZ, Origin ZX.
   - Bottom-left count of instances needing creation; **Update** button creates missing derived parts/assemblies (may auto-create versions in linked and current docs). Warning [BUNDLE]: "Some instances require new components to be created. Verify the mirror strategy to create new components."
- Result: Mirror feature in Instances list; mirrored instances nested under it. Mirrored subassembly motion follows original (mates not recreated). Instances can be added later. Symmetry change does not auto-change Mirror action. Mirror only fully constrained subassemblies recommended. Configurations propagate (view-only in derived tabs).

---

## 14. Replace instance [DOCS]

- Dialog: **Instances** to replace (one or many) -> **Select replacement component** field opens browser (Current document / Other documents, Part Studios / Assemblies, search with filters - same as Insert) -> picking an item closes browser -> **Replace all instances** checkbox (replace every instance of the same component) -> checkmark. Mates reapplied to the replacement where possible.
- Bundle labels: "Replace instance", "Selection to replace", "Selection to replace with", "No replacements found."

---

## 15. Display states [DOCS][BUNDLE]

- Toolbar Display states opens a small dialog: **display state dropdown** ("Select a display state"; empty: "No display states created"), name field ("New display state name"), **+** "Add display state", **Update** "Update this display state", **pencil** "Rename display state" (Enter or click out to accept), **delete** "Remove display state" (immediate).
- Captures visibility of parts, subassemblies, mates, MCs. Select from dropdown to apply.
- New items added after a state was created: explicit MCs visible by default in old states; implicit MCs and mates hidden by default.
- Drawings default to "Show all"; drawing views can pick a display state; changing display state flags drawing update. Messages: "Display state updated/renamed/deleted.", "There is already a display state with this name.", "A display state must be selected for update/removal".

---

## 16. Exploded views [DOCS]

Panel opened from right-side **Exploded views** icon (or instance list assembly menu "Add exploded view").
- **Add exploded view** button (top when empty, bottom afterwards) -> creates *Explode 1* row with a **rollback bar** beneath it and enters **Explode mode**: gold banner ("Editing __explodedViewName__" [BUNDLE]; hint "Build steps by dragging components" / "Select components in the graphics area or the instance list, then drag or rotate them using the triad manipulator." [BUNDLE]) with **Done**. Mates/patterns/relations cannot be created in Explode mode.
- Select instance(s) (click / box select) -> triad manipulator appears; manipulator origin can be dragged to reposition (snaps to MCs). Drag an arrow/plane/arc -> on release an **Explode step** dialog opens and step is created under the Explode.
- Explode step dialog fields:
  1. **Instances** (editable).
  2. Movement type: **Translation** | **Rotation**.
  3. Translation: **Specify direction** checkbox -> direction entity (face/edge). Rotation: **Specify axis** checkbox -> axis entity. Without these the reference is the manipulator/instance.
  4. **Translation distance** (or rotation degrees); negative allowed; manipulator drag updates it.
  5. **Explode lines** checkbox (default on).
  6. **Edges, vertices, or face for explode lines** - by default lines start at the centroid of the selection; picks set per-entity line anchors. Lines inherit the previous step's selections.
  7. Checkmark accepts step. Selection persists after commit -> drag again to add another step for the same instances; clicking an unselected instance replaces the selection.
- Panel tree: Explode > steps; context menu on explode: **Activate** (display & fit), **Rename**, **Show/Hide explode lines** (view or step), **Reset starting position**, **Duplicate**, **Delete**; on step: Rename, Edit (or double-click), Show/hide lines, Delete; drag steps to reorder or into another explode.
- Rollback bar: drag to view intermediate state (steps below suppressed); click bar then Up/Down arrow keys to step.
- Configurable: right-click a step field > Configure (field outlined yellow dashed). Config missing instances -> step + explode turn red ("One or more explode steps contains an error" [BUNDLE]).
- Follow mode shows exploded view activity. Drawings: Insert view > Explode/Position dropdown picks an explode (all steps shown regardless of rollback). Multiple Explode features = multiple drawing choices.

---

## 17. Named positions [DOCS][BUNDLE]

- Panel from right-side **Named positions** icon. Table: rows = named positions (Name column), extra columns = driving mate DOFs.
- **Add named position** ("Add a new named position for this assembly") -> new row "Named position 1" + yellow banner (edit mode: "Position instances in the graphics area and define driving mates.") with **Done**. Button moves to bottom once a row exists.
- In edit mode: drag instances (unmated instances store absolute transforms); click a mate (list) or a mated part -> **Driving mate selector** dialog with checkboxes for each DOF of the mate; each checked DOF adds a column ("__mateName__ __dofType__ added to named positions"). Outside edit mode: mate context menu **Edit named position driver mate** (all DOFs preselected).
- Column header context menu: Move column to the right / left, Move column to start / end, Delete ("Delete degree of freedom").
- Row (Name cell) context menu: **Apply named position**, **Edit named position**, **Duplicate named position** ("Copy of" prefix), **Update named position**, **Move up**, **Move down**, **Delete named position**.
- Cells editable numerically, then Apply to drive. Exit edit: Done or click in table.
- Subassemblies: right-click subassembly > **Add to named positions** -> top-level gets a column with a **Position** dropdown choosing a subassembly named position (auto-creates "Position A" if none). "Delete subassembly named position" [BUNDLE].
- Failures: "some Mate values could not be applied" (deleted parts, failing mates, DOF outside limits). Fastened/Tangent positions not stored. Drawings: view context menu Explode/Position > named position. In-context: contexts can follow a named position.

---

## 18. Modeling in context

### 18.1 Edit in context [DOCS]
- Instances list/graphics right-click part > **Edit in context** (submenu: existing contexts or **Create new context**). Not allowed for revision references ("Cannot edit in context" [BUNDLE]); version references prompt **Select a workspace to edit** dialog.
- Opens the part's Part Studio with that part as **primary instance**; other assembly instances shown ghosted/transparent in assembly relative position; view orientation, zoom and section view carried over (and back).
- Selecting any ghosted geometry as a reference creates an **Assembly context** (snapshot of geometry + positions); contexts list ("Assembly contexts") appears above the Feature list; referenced entities highlighted purple/pink; features using context show a context arrow icon.
- Yellow banner: **Hide/show instances** dropdown (incl. **Go to assembly**), transparency slider, **Done**, X (exit without creating context). **y** hides selected context parts. Context menu: Hide context instance(s), Restore context view state. Context three-dot menu: **Rename**, **Update context**, **Change configuration**, **Hide/show context instances** (dialog with filter + eye icons), **Follow position** (current or named position).
- Selecting **None** in contexts list turns context off.
- Updates never automatic: Assembly right-click part > **Update context** > pick context; or in Part Studio context menu > Update context. Out-of-date shown by blue filled dot on contexts label/entries and on assembly instance/folders ("Update available for __contexts__", "Updates available for contexts in assembly/folder").
- **Set as primary instance** (<context> > Set as primary instance): solid arrow = primary; dashed arrow = previous/not primary.
- References never break ("parts never fail").

### 18.2 Create Part Studio in context [DOCS]
- Tool dialog: single selection - an explicit Mate connector or the Assembly origin ("Select origin/mate connector" [BUNDLE]) -> checkmark.
- New Part Studio opens with default planes aligned to the selected MC/origin; assembly shown dimmed as context; message bar with dropdown **Insert and go to assembly** / **Go to assembly**.
- Insert and go to assembly: pick created part(s) -> checkmark -> inserted as primary instances (solid arrow). Inserting later via Insert shows dashed arrow.

---

## 19. Updating references (linked documents) [DOCS]

- Toolbar **Update all references** button: active when updates exist for non-pinned references (recursive through subassemblies; auto-creates versions as needed; every reference updated to latest).
- Instances list icons: chain/version icon (up-to-date link), blue background (newer version on branch with edits to that tab), triangle (revision), thumbtack (pinned), open grey circle (pinned + newer), solid grey circle (child has newer; navigate down), small arrow blue/grey (subassembly contains out-of-date child). Out-of-date propagates up.
- Click a link icon -> **Reference manager**: tabs/filters **Update to latest** and **Selective update**; checkbox per object; version graph icon to pick any version (incl. older); **Update selected**; filter icons for Pinned and Revision; "Update to the new document" for moved tabs; prompt to choose branch/workspace if multiple. Disabled if missing write permission.
- Right-click link icon > **Pin reference** (exclude from Update all). Instance menu > **Change to version...**.
- Revisions must be updated in the subassembly where referenced first, then pulled up.

---

## 20. Snap mode [DOCS]

- Toggle (Shift+S); highlighted when on.
- With instances already inserted: click-drag an instance -> it becomes transparent; dragging near another instance wakes its explicit/implicit MCs; cursor changes when a snap is possible; Ctrl cycles the dragged instance's explicit MCs; **a**/**q** change alignment during drag (flip primary / reorient secondary); release -> Mate dialog opens with both MCs filled (default Fastened [INFERRED from insert behavior]) to choose type, flip, reorient, animate, accept.
- With Insert dialog: see 4.3 (creates Fastened mate).
- Zoom/rotate/pan allowed during snap drag.

---

## 21. Show mates mode and mate visibility [DOCS]

- **h** toggles Show mates mode: hovering an instance (graphics or list) shows its mates (all levels, through subassemblies); click an instance to pin its mates visible.
- **j** shows/hides all mates (hidden by default). Per-mate eye icon. Select mates > right-click > Show/Hide; on an instance right-click > Show mates.
- Glyph context menu includes Edit.

---

## 22. Assembly Variable table [DOCS]

- Right-side **Variable table** panel (with BOM, Configuration, Exploded views, Named positions). Lists Variable Studio tables referenced by the assembly.
- Bottom dropdown: **Create Variable Studio** (new Variable Studio tab; table editable inline) / **Insert Variable Studio** (dialog to pick from this or other document; other-document tables read-only; require a version).
- Row fields: **Name** (FeatureScript identifier; duplicates auto-renamed `Length_0` with message), **Type** dropdown, **Value**, **Description**. Enter/Tab commits.
- Table ellipsis menu: Copy table, Switch to Variable Studio [x], Open linked document, Update linked document, Move up/down, Edit reference, Remove reference. Cell context menu: Delete, Move up/down.
- Same-name variables: last table wins; override icon on overridden variable. Renames propagate to features. Assembly variables drive mate offsets/limits/pattern values ("Add variables to drive mates and part configurations in the assembly." [BUNDLE]).

---

## 23. Bill of Materials (BOM) [DOCS]

### 23.1 Panel
- Right-side **BOM Table** icon. Auto-populated, live-updating with assembly and properties.
- Header controls: **BOM type** (Flattened | Structured), **Insert items** (non-geometric items: dialog with search, Classification filter, quantity, Insert; items appear in Instances list Items section; item context menu Edit qty, Suppress, Add comment, Delete), **Add column** dropdown, **Apply template** (Pro/Enterprise/EDU), **Overflow (...)** menu.
- Overflow menu: Copy table; Show/Hide excluded/suppressed; Collapse all / Expand all (Structured); Show/Hide top level assembly row (aggregated values at bottom); Save as template; Export to CSV (unexpanded subassemblies not exported).
- Default columns: **Item**, **Quantity**, **Part number**, **Description**. More from properties (Name, Material, Vendor, State, Revision, Not revision managed, custom props...).

### 23.2 Behavior
- Item numbers follow Instances list order (reordering list renumbers). Structured: subassemblies with caret; double-click Item cell to expand; children numbered `9.1`, `9.2`. Flattened: all parts as if top-level.
- Hover row -> highlights in graphics; click item -> cross-highlights in Instances list and graphics; selecting in list scrolls BOM.
- Item column frozen on horizontal scroll.
- Editable property cells write through to part/assembly properties (subject to edit-in-workspace/version permissions; released/imported read-only; standard content greyed). Material cell double-click -> material picker.
- Column header context menu: Remove column (not Item), Move left / Move right; Part number header: **Generate missing part numbers**; cell: **Generate next part number**. Resize by dragging header edge. Sort: double-click header cycles unsorted -> ascending -> descending (one column at a time; not Item).
- Row context menu: **Switch to Part Studio/Assembly** (same doc) or **Open linked document**; **Suppress from this BOM** / **Unsuppress in this BOM** (Structured only; immediate children only; shown as `--*` when visible); **Exclude from all BOMs** / **Include in all BOMs** (global; shown as `--`). Aggregated computed property override dropdown + **Remove override**.
- **Subassembly BOM behavior** (assembly Properties): Show assembly and components (default) | Show assembly only | Show components only.
- Templates: **Create BOM template** dialog - Name (required), BOM Type, Property columns (Item required), All uppercase in Drawings, Show top level assembly row; Create / Cancel.

---

## 24. Interaction behaviors (graphics area)

### 24.1 Moving instances [DOCS]
- **Free drag**: click-drag an instance; it moves only within its remaining DOFs (solver drags mated chains; limits stop motion). Fixed instances cannot be dragged: visual "cannot move" cue, no manipulator.
- **Manipulator drag**: click instance -> **triad manipulator** at instance (or last placement). Parts: 3 axis arrows (translate along), 3 plane squares (translate in plane), 3 arc handles (rotate about X/Y/Z), center ball (origin).
  - Drag triad origin to reposition manipulator without moving part; snaps to inferred/explicit MCs; then manipulator motions are relative to that point.
  - During drag a numeric entry field appears; type exact distance/angle.
  - Right-click center: **Move to origin** (moves part reference point to assembly origin; no mate/fix).
  - Right-click axis arrow: **Align with Z**, **Anti-align with Z**.
  - Right-click arc: **Rotate 90 degrees**, **Rotate 180 degrees**.
  - Unmated & unfixed instance moves exactly; mated instance tries within DOFs; solver may fail to find a solution -> reposition manipulator or try another handle.
- Positions are not locked after drag (unless fixed/mated); **Reset** on mate returns to home.

### 24.2 Solving [DOCS]
- All mates solved simultaneously on drag/accept. During mate creation only current mate's instances move until **Solve**.
- Over-constrained/unsolvable mates turn red (glyph + list icon); list shows error with expandable location; hover for message. Fastened-to-origin while fixed parts exist -> warning. Multiple fixed parts -> warning.
- Rigid detection: all-zero-DOF + one fixed => rigid icon (performance benefit in parents).
- Subassemblies: flexible by default (subassembly mates respected and drag-able in parent); Lock/follow position freezes.

### 24.3 Fix / Unfix [DOCS]
- Right-click instance (graphics or list) > Fix / Unfix; pin icon. Unfix all from assembly menu. Fix is local to current assembly.

### 24.4 Hover/selection [DOCS]
- Hover a mate in list: associated instances highlighted, glyph shown. Select a mate/MC/relation in graphics: highlights list rows.
- Click an instance: list scrolls to it; BOM row highlights.
- During mate creation: implicit MC dots on hover; triad preview at nearest point; Shift locks; Ctrl cycles in snap.
- Selecting a mate shows frame axes, offsets and limit graphics with live values (double-click value to edit temporarily).

### 24.5 Visibility shortcuts
- y hide hovered/selected; Shift+Y show hidden; k MCs; j mates; h show mates mode.

### 24.6 Isolate / Make transparent dialogs [DOCS]
- Shift+I / Shift+T or context menu (instances, faces, edges, MCs, folders, groups, mates).
- Dialog: **Transparency slider** ("Show/hide internal geometry"): left = only outer edges of transparent bodies; middle = internal geometry visible; right = fully invisible. **Select transparent geometry** checkbox ("Enable selection of parts, faces, and edges from transparent bodies"). **Component controls**: **Expand: distance** | **Expand: connectivity** (Assembly only; by mate connectivity) + slider ("Expand isolate/transparency by physical proximity / mate connectivity" tooltips). Close X or Esc restores; settings persisted across sessions. Isolate mode: "Exit isolate" at top of context menu.

### 24.7 Copy/paste
- Ctrl+C / Ctrl+V instances; pasted at click location; mates & MCs duplicated.

---

## 25. Implementation notes for noshape (derived)

- Data model: Assembly = ordered instance tree (instances, folders, subassembly refs w/ configuration + version/workspace ref, patterns/replicates/mirrors as generator features) + ordered Mate-features list (MCs, mates, groups, relations, folders). Mate = {type, mc1, mc2, flipPrimary, secondaryAxisType(+X,+Y,-X,-Y), offset{x,y,z,rotAxis,rotAngle}, limits{per DOF min?, max?}, suppressed, hidden, name}.
- Mate solve state per DOF value (for named positions, animate, apply limit position, live value display).
- Implicit MC = stored inference reference {entity query, inferenceType, flip, reorient, optional realign/move}, re-evaluated on geometry change (geometry change moves mates).
- Error/warning taxonomy and tooltips in 5.3 can be ported verbatim.
