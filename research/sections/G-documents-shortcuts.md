# G. Documents, Tabs, Versioning/History, Import/Export, Keyboard Shortcuts

Sources: Onshape Help (Document__*, File__*, Collaboration__*, Home__keyboard_shortcuts_and_hotkeys, Home__user_interface_basics, Home__glossary, Assembly__updating_references, FeatureStudio__feature_studios, Library__*) tagged [DOCS]; client bundle command IDs / i18n strings tagged [BUNDLE]. Nothing here was observed live.

## 0. Clone mapping (noshape desktop) — summary

| Onshape concept | Suggested noshape mapping |
|---|---|
| Documents page (cloud list, filters, folders, labels, trash) | Start/"Open project" screen: recent projects list, filesystem folders, optional tags, OS trash or app-level trash |
| Document (container of tabs, own history) | One project file/bundle (e.g. `.noshape` dir or SQLite) holding all tabs + full change log + versions/branches |
| Tab (element): Part Studio, Assembly, Drawing, Variable Studio, Feature Studio, imported file (blob) | Element records inside the project; bottom tab bar |
| Workspace / version / branch | Mutable branch heads / immutable named snapshots / branch DAG stored in project (git-like) |
| History entry (every persisted edit) | Append-only change log per workspace; each entry `Tab :: Action : Feature` |
| Linked documents (insert from a *version* of another document) | Cross-project references pinned to a version id of the other project; "newer version available" indicator |
| Share / follow / comments / App Store / Learning Center / release mgmt | Out of scope (comments optional, local-only) |
| Browser download / "store file in a new tab" | Save-file dialog / embed exported blob as a new tab |

---

## 1. Documents page (moderately brief) [DOCS]

Landing page after sign-in (Enterprise lands on Activity page). Onshape/company logo in any document returns here. Layout and filter/view choices are remembered across sessions.

### 1.1 Layout (numbered areas)
1. **Search field** – searches within the currently selected filter/folder. Results columns: Name, Modified, Modified by, Part number, Revision, State (last 3 only if present). Name and Modified sortable by header click. Down-arrow in field opens **Advanced search** (§1.6).
2. **Create button** – menu (§1.2).
3. **Left side panel** – Filters group, Integrations (Google Drive / Dropbox / OneDrive accounts, shown above Trash), **Trash**.
4. **Last opened by me** strip – thumbnails of your recently opened docs (when company/enterprise filter selected); collapsible.
5. **Projects and Folders** section – grid (default) or list view toggle; **Sort** button; **Type** (Enterprise: folders/projects/both). Collapsible; divider between it and documents list draggable, position persisted.
6. **Documents and Publications** section – Views: **Document view** (default flat list; Type: Show all / Documents / Publications), **List view** and **Structure view** (release-data views; Pro/Enterprise; out of scope). Expand/collapse control collapses both areas. **Add** button opens column chooser: checkboxes per column, Sort icon to reorder columns, search box.
7. **Right side panels** (toggle icons): **Details** (resizable, min 340 px, width persisted), **Analytics** (Document Dashboard: modeling time, actions, tab types, activity, access locations), **Share details**, **Properties** (metadata; admin-set props greyed), **Versions and history** (full graph actions available from here), **Document notes**. Clicking doc name in a panel opens it in new browser tab; hovering shows Copy link icon. Revision-history icon appears when results include release info.

### 1.2 Create menu
- **Document** – name + optional existing label; *Document location* field to choose folder (or navigate into folder first). Company members choose owner (default company). Also `cad.onshape.com/new`.
- **Folder** – name + optional parent folder.
- **Publication** – (collection of released items; out of scope).
- **Import files** – from local disk (see §4).
- **Import from** – Google Drive / Dropbox / OneDrive ("Connect service…" wizard if none).
- **Project** – Enterprise only.
- **Label** – create a label.
- Create is disabled when user has read-only access to the current folder.
- Drag-and-drop files onto the Documents list (blue import outline) also imports. [DOCS]; "Drop to import file to __container__" [BUNDLE].

### 1.3 Filters (left panel)
Company name filter (all docs you created + shared with company) · **Recently opened** (context-menu "Remove from recently opened") · **Created by me** · **Shared with me** · **Teams** (collapsed group) · **Labels** (user-specific, collapsed group; "+" create-label icon) · **Public** (columns: Name, Modified, Owned by, Copies, Links, Likes) · Integrations · **Trash**.

### 1.4 Selection, navigation, inline + context commands
- Click tile/row = select (name background blue); click the *name* (underlined on hover) = open.
- Arrow keys move through list; **Ctrl+click** multi-select; **Shift+click / Shift+arrow** range; *Deselect all* button in Details panel.
- Drag documents onto a folder to move (Document view only); drag onto a label to apply label; drag onto Trash to delete.
- Inline row icons: **Share**, **Send to trash** (also **Delete** key), **Labels**.
- Context menu (varies by type/selection/permissions): Open · Open in new browser tab · Rename document/folder/publication · **Copy workspace** (dialog "Copy active workspace to new document": name + folder browser with *New folder* and *Sort*; toast with link to copy) · Details · Versions and history · Labels · Share · **Move to** · Go to location (released items; Document/List/Structure view) · Transfer ownership · Unshare with me · Make public (not Enterprise) · Make personal copy · **Send to trash** · Remove from recently opened.
- Thumbnail: document thumbnail = last Part Studio opened, unless a tab is chosen via tab context menu **Select as document thumbnail** (needs Edit; reverts to most recently edited tab if that tab is deleted/moved). [BUNDLE] toasts: "Document thumbnail set to __elementPartName__", "Document thumbnail cleared".

### 1.5 Folders, labels, trash
- **Folders**: Create > Folder (name, optional parent). Breadcrumbs above Projects-and-Folders; click breadcrumb name to rename. Folder context menu: Open, Open in a new browser tab, Rename, Details, Share, Unshare with me, Move to, Transfer ownership, Send to trash (contents go too), Set library (frame / sheet metal forms / images library). Nested folders; drag folder into folder. **Move to** dialog: folder browser, *New folder*, *Sort*, *Show details* (list of items being moved), double-click to enter, **Move here**. Share permissions inherited from root folder, additive. "Cannot exceed __max__ levels of folder nesting." [BUNDLE]
- **Labels**: Create > Label, or "+" next to Labels filter, or Labels icon on a row → Create new label dialog (name, Create). Apply via context menu *Labels* (checkbox list with search) or drag onto label; multiple labels per doc; not applicable to folders. Right-click label > *Delete label* (only removes label from docs).
- **Trash**: only items you trashed; right-click > *Restore* or *Delete* (permanent); **Empty trash** button; auto-purge after 30 days (90 days Enterprise). Docs in trash can't be opened by links ("Cannot open a document in the trash. Restore the document from Trash.").

### 1.6 Advanced search (down-arrow in search bar)
Fields: **Type** (All, Document, Part, Part Studio, Assembly, Drawing, Feature Studio, Variable Studio, Project, Folder, Publication) · **Category** (multi) · **Name** · **Description** · **Part number** · **State** · **Revision** · **Add criteria** (Owner, Creator, Vendor, custom properties; each adds another row; dates show calendar, booleans True/False) · **Results from** (Workspaces / Workspaces and Versions / Versions; latest or all; current folder ± subfolders). Wildcard `*`. **Reset** clears. Query auto-populates the search bar. Result row: type icon, name, workspace/version/revision icon, workspace/version name in parentheses, part number (grey), state. Result context menu: Open, Open in new browser tab, Export, Details, Share, Go to location, Transfer ownership.

### 1.7 Notifications (bell icon, title bar)
Count of unread; auto-deleted after 14 days. Types: comments, upload status, export status, workspace status (out of date after branching/restoring from a version created before last Onshape update). Import/export progress, warnings, errors and a **retry** link for failed imports appear here.

**Clone note**: the desktop clone needs only: recent list, open/new/import, folder browse (native dialogs), rename, move, delete-to-trash, copy (= "Copy workspace" → Save As new project), thumbnail per project, search by name/part-number/description.

---

## 2. Document structure

### 2.1 Anatomy [DOCS]
- **Document toolbar** (top row, aligned with logo) – §2.2.
- **Document panel** (vertical strip at left edge) – §2.4.
- **Toolbar** of the active tab (Feature/Sketch/Assembly/Drawing toolbars) — last-used tool in a dropdown group moves to top of its group for the session; reset on reopen. Toolbars collapse into dropdown groups at small widths. *Search tools* (Alt+C) finds any tool (not in Document/Drawing toolbars).
- **Feature list / Instances list** (left), **graphics area**, right-side panels.
- **Tab bar** (bottom) with **Insert new tab "+"** and **Tab manager** icon at bottom-left.
- New document default content: **Part Studio 1** and **Assembly 1** tabs; **Start** version + **Main** workspace auto-created (both renameable). Opening a document activates the last tab you used.
- Recommendation: >~100 tabs → split into linked documents (performance). [BUNDLE] warnings: "__documentName__ contains many document tabs / Part Studio tabs / Assembly tabs."

### 2.2 Document toolbar — every item [DOCS]
| Item | Behavior |
|---|---|
| Onshape / company logo | Return to Documents page |
| **Document menu** (hamburger) | Opens Document menu (§2.3) for active workspace/version |
| **Document name** (bold) | Click to rename inline |
| **Workspace name** (light grey) | Name of active workspace (or version) |
| **Copy link** | Copies URL incl. active configuration, render mode, exploded view, panel states, view orientation. Toast "Document link copied to clipboard." [BUNDLE] |
| **Folder name** (folder icon) | If doc is in a folder; link opens the folder in a new browser tab |
| **FeatureScript notices** | Compiler notices list (filterable: "Filter FeatureScript notices" [BUNDLE]) |
| **Notifications** (bell) | Document change notifications, import/export queue |
| App Store | out of scope |
| Learning Center | out of scope (external link) |
| **Share** | Share dialog (out of scope) |
| **Help menu** (?) | Online Help, Learning Center, FeatureScript documentation, **Keyboard shortcuts**, What's New, Forums, Contact Support, Refer a friend, Check system compatibility, About |
| User account menu | My account / Preferences (keyboard shortcuts customization lives here), sign out |
| Public docs only | Public indicator, **Copies** count, **Links** count (number of documents linking this one), **Likes** (click to like) |
| Social cues (collaborator avatars) | double-click to follow (out of scope) |

### 2.3 Document menu — every item [DOCS][BUNDLE]
Workspace variant vs version variant (menu differs):
- **Rename document** – new name.
- **Move to…** – folder picker. Moving across root folders transfers ownership to folder owner and re-inherits share perms.
- **Document details** – Description (shown in Details panel), *Not revision managed* checkbox (release mgmt only), Category.
- **Restore deleted workspaces** – dialog listing deleted branches (checkboxes) → **Restore selected (n)**. Workspaces restorable within 30 days of deletion [BUNDLE]. "No deleted workspaces found in this document." [BUNDLE]
- **Copy workspace…** – (workspace mode) "Copy the active workspace to a new document": new doc, not linked; opens automatically to same tab; choose folder. Copy goes to root if no edit permission on folder.
- **Copy version** – (version mode) "Copy the active version to a new document"; no release metadata copied (except part numbers).
- **Update workspace** – upgrade workspace to latest Onshape feature-library version; dialog **Update now**; doc inaccessible during update (update icon on Documents page). Repeat per workspace.
- **Workspace units** – §2.8.
- **Workspace properties / Version properties** – tree of tabs and parts with Name, Description, Part number, Revision, State, etc. (auto part number icon when release mgmt).
- **Switch to view only mode** – hides toolbars, navigation only; banner "You are in view only mode" with **Edit** button to return.
- **Print** – print graphics area of the active tab ([BUNDLE] "Print setup", "Print image...").
- **Close document**.
- iOS variant adds Workspace properties / Copy workspace / Workspace units (same functions).

### 2.4 Document panel (left vertical strip) — every item [DOCS]
| Icon | Opens |
|---|---|
| **Versions and history** | Version graph panel (§3) |
| **Create version** | Create version dialog without leaving workspace (§3.3) |
| Release drafts | out of scope (release mgmt) |
| **Comments** | Comments panel (blue dot = new comments; number = comments assigned to you) |
| **Document notes** | Markdown notes panel (§2.7) |
| **Where used** | Where-used panel (§3.12) |
| Performance | Tab performance, Tab graphics, Document performance, System details (optional for clone) |
| AI Advisor | out of scope |
| Explore Onshape | out of scope (tutorial) |
| Action items | tasks (out of scope) |
| **Tab manager** | Tab manager panel (§2.6) |

Mobile: Versions and history, Create version, Comments, Share on a bottom toolbar.

### 2.5 Tabs (elements)
**Tab types** [DOCS]: Part Studio · Assembly · Drawing · Variable Studio · Feature Studio · imported files (each import = one file/blob tab; CAD imports additionally create Part Studio/Assembly tabs; tab icon shows file type) · Material library · folders (organizational) · out of scope: Render Studio, PCB Studio, CAM Studio, App tabs ("Data from application tabs may be saved outside Onshape").

**Insert new tab "+" menu** (bottom-left) — items in order shown in docs:
1. Add application (App Store apps) — out of scope
2. Create Material library
3. Create Feature Studio
4. Create Render Studio — out of scope
5. Create PCB Studio — out of scope
6. Create CAM Studio — out of scope
7. **Create Part Studio**
8. **Create Assembly**
9. **Create Variable Studio**
10. **Create Drawing** (opens template dialog — see Drawing section)
11. **Create folder**
12. **Import** (file picker → import options dialog §4.2)
- Also paste target: a tab copied with *Copy to clipboard* is pasted via this "+" menu ("Paste __name__" [BUNDLE]; "Tab failed to paste." on error).
- New tab is placed **directly right of the active tab** and becomes active immediately. Default names "Part Studio N", "Assembly N", etc.

**Tab bar behaviors** [DOCS]
- Click tab = activate (one active tab per window; open others in new browser window/tab via context menu, Ctrl/Cmd+click, Shift+click).
- Hover a tab = thumbnail preview.
- Drag to reorder; **tab order is shared by all users and persistent**; active tab and scroll state are per-user and not persisted (except "previously active tab" restored on reopen; first open → first tab).
- Active tab kept in view when scrolling; active tab = blue border/highlight.
- Rename: double-click name or context menu *Rename*. Validation [BUNDLE]: "Tab name cannot be empty.", "Tab name cannot be longer than __maxLimit__ characters.", "Tab name cannot contain HTML." History entry/toast "Renamed tab __old__ to __new__".

**Tab context menu** [DOCS][BUNDLE] (right-click a tab; Workspace vs Version):
| Item | Workspace | Version | Notes |
|---|---|---|---|
| Open in new browser tab | ✓ | ✓ | |
| Rename | ✓ | – | |
| Properties | ✓ | ✓ | Name, Description, Part number, etc. ("Failed to edit tab properties." on error) |
| Show code (FeatureScript) | ✓ (Part Studio only, view-only) | ✓ | "The FeatureScript representation of this Part Studio is view only." |
| Duplicate | ✓ | – | non-associative copy, placed next to source |
| Copy to clipboard | ✓ | – | then paste into any document via "+" menu |
| Select as document thumbnail | ✓ | – | |
| Move to document… | ✓ | – | §2.6.2 |
| Export… ("Export tab…") | ✓ | ✓ | §4.5 |
| Download | ✓ (imported-file tabs) | ✓ | original file bytes, same name/type |
| Update (imported file) | ✓ | – | replace blob with new file; reruns dependent Import features |
| Change to version / Update linked document… | Drawing/linked tabs | – | switch references to a version / newer version |
| Where used | Part Studio/Assembly/Feature Studio | ✓ | §3.12 |
| Revision history | released items | ✓ | out of scope |
| Create Drawing of __name__… | Part Studio/Assembly | – | |
| Create task | ✓ | ✓ | out of scope |
| Delete | ✓ (even active tab) | – | §2.6.3 |

**Folder (tab-bar folder) context menu**: Rename · Move to parent folder · Unpack folder (move contents up, delete folder) · Move to document. When a folder is active, the tab bar shows only its tabs plus an **All tabs** home icon; nesting up to 10 levels; >1 level deep shows an overflow tab listing parent folders. Drag tabs onto folder to file them; drag folder onto folder to nest.

### 2.6 Tab manager [DOCS]
- Opens as a left panel (pushes Feature/Instances list right). Shortcut **Alt/⌥+T** (static ID `OPEN_TAB_MANAGER`).
- Header icons: **Search tabs** field toggle (type-ahead filter by name) · **Tab order selection** (Tab order in document / A–Z / Z–A / by type; last choice remembered) · **Display filter icons** toggle (type filters: Part Studio, Assembly, Drawing, File, Released… only types present are shown; additive; **Clear** button) · **List view** (names only + type icon) · **Detail view** (names + details + thumbnail per row).
- Thumbnail of the selected tab at the bottom.
- Click name = open tab; drag to reorder (shared order); **Ctrl+click** multi-select → context menu: Move to document, Add selected to a folder, Delete (multiple), Export (multiple Part Studios OR multiple Assemblies OR multiple Drawings — not mixed: "Export drawings").
- With filters off, folders are visible; folder context menu: Rename, Add selected to a folder, New folder, Unpack folder; any item: Add selection to folder.
- [BUNDLE] "Tabs are filtered, some results may not appear in search.", "The new tab is not visible in the tab manager with the selected filters."

#### 2.6.1 Quick tab switching (Ctrl+Space)
Hold **Ctrl**, press **Space** repeatedly to cycle thumbnails of previously opened tabs (MRU order, single session); release both to open the highlighted tab (ID `OPEN_ELEMENT_TAB_SWITCHER`).

#### 2.6.2 Move to document
- Context menu on tab(s)/folder → choose existing or **new** document (created during operation).
- Moved entity is removed from source; a **link** is created from the original doc to the moved entity (assemblies keep working via linked references).
- Moving an Assembly also moves referenced Part Studios unless *Move all referenced tabs* is unchecked. Move blocked (red message) if it would leave the source with no tabs.
- Requires Edit + Link on target doc. [BUNDLE] "Moving __n__ tabs to __newDocumentName__.", "Tab cannot be moved into a protected workspace. Create or select a branch to move it into."
- Recipe: share only a drawing → version the model → drawing context menu *Change to version* → move model or drawing out.

#### 2.6.3 Deleting tabs
- Right-click > Delete. If deletion would break references, **Delete tabs** warning dialog lists conflicts → **Delete** / **Cancel**; otherwise deletes immediately with no prompt. Deleting imported file tabs warns which Part Studios are affected. Undoable via undo/history ("Deleted __n__ document tabs" [BUNDLE]).

### 2.7 Document notes [DOCS]
- Panel header: **Refresh notes**, **Edit notes**, **X** close. Empty: "No notes in this document" [BUNDLE].
- Editor: **Apply**, **Cancel**, **Preview** (renders in lower half), **X**.
- Content: standard Markdown (setext/ATX headings incl. closing #'s, blockquotes incl. nested, lists `* + -`, ordered lists, code, links). Shared per document (also shown in Documents-page Notes panel). Clone: store as Markdown string in project metadata.

### 2.8 Workspace units [DOCS][BUNDLE]
- Document menu > **Workspace units** dialog; ✓ accept / ✗ cancel. Defaults from account Preferences (Onshape default inch/degree).
- Affects all Part Studios and Assemblies of that workspace, sketch dimension display and default input units of all feature fields. Not drawings (drawing units separate; *Feet and inches* only in drawings). Does not affect imported files.
- Unit rows (labels in bundle): Length, Angle, Mass, Time, Volume, Area, Energy, Force, Pressure, Moment, Acceleration (mobile docs list Length/Angle/Mass) + **Workspace precision** (decimal places per quantity).
- Precision rules: display only (values stored at full precision, rounded only on display); applies to feature dialogs, sketch dimensions, manipulator boxes, Measure (switches to scientific notation if precision insufficient), Mass properties; extra digits typed remain visible while the field is focused.
- Any numeric field accepts explicit units ("10mm") and converts to the default unit. New workspaces branched from a version inherit that version's units. Toast "Default units updated." [BUNDLE]

### 2.9 Properties [DOCS]
Properties dialogs: Parts (Parts-list context menu), Part Studio/Assembly/file tabs (tab context menu), workspace/version (Versions panel context menu > Properties: Name, Description editable; Category and tab tree shown; each tab selectable/editable). Version geometry is immutable but version *properties/metadata* remain editable.

### 2.10 View only mode [DOCS]
Docs may open in View only mode (company setting / no edit permission): toolbars hidden, banner "You are in view only mode" + **Edit** button. Re-enter via Document menu > *Switch to view only mode*. Versions and history-moments are inherently view-only (§3.6).

---
## 3. Versions, branches, history

### 3.1 Concepts [DOCS]
- **Workspace** – mutable modeling space; one per branch at its tip. Every *persisted* edit by any user in any tab is recorded (no "Save" command exists).
- **Version** – immutable named snapshot of the entire document (all tabs) at a point in time; geometry/part names frozen, version properties editable. Default names **V1, V2, …** (auto-increment); renameable. **Start** version created with the document (contains the initial empty Part Studio + Assembly).
- **Branch** – named fork created at a version; has 0..N versions and at most one workspace at its tip. Default workspace **Main**. Branch names "B1…" style in examples.
- **History entry / change** – record of one change to a workspace; text format `Tab-name :: Action : Feature-name` (e.g. `Part Studio 1 :: Insert feature : Fillet 1`). Hover shows user + timestamp.
- Automatic versions – created by Onshape when updating references (Update all), shown with a special icon.
- Release states (In progress / Pending / Rejected / Released / Obsolete) and release candidates – out of scope (only the graph icons matter).

### 3.2 Versions and history panel (graph UI) [DOCS][BUNDLE]
- Opened from Document panel (or Documents-page panel). Title "Versions and history of __documentName__".
- **Panel toolbar** (top): Create release (out of scope) · **Create version** · **Compare** (pick two entries) · **Repair** (View in repair: pick a change) · **Active branch / All branches** toggle · **Filter branches** · **Show/Hide merges** · **Show/Hide automatic versions** · **Search history** field with dropdown.
- **Graph**: tree, color-coded per branch, newest at top. Workspace = **open dot** at branch tip; version = **solid dot**, normal text; branch title = **bold italic**; closed dot on a branch = its workspace was deleted. Releases = triangles (solid released / open pending / solid-with-dot obsoleted). Active workspace = blue box at left of its row / dark-blue highlight. Merge lines drawn explicitly between branches. Version containing published FeatureScript shown with `{}` around node. Lock icon on protected workspace. Onshape-employee edits show Onshape logo next to user in *Modified* column.
- Columns: name, Modified (user/time). Hover a version → its description tooltip.
- **Show changes** expanders: under each workspace/version a collapsible "n changes" group listing history entries; first **25** shown, then **Show more**. [BUNDLE] "Show more changes", "Show changes in history".
- Legend icons: version · change node · version with approved release · version with release containing obsoleted item · workspace · version with revision pending · automatic version.
- **Filter branches** options: Show all branches (default) · Show all branches with workspaces (hide branches of deleted workspaces) · Show current branch · Show current branch with parent branches.
- Branch context menu extra: **Scroll to branch top / Scroll to branch bottom**; "Show __parentName__ branch history" [BUNDLE].
- **Search history**: free text over history entries across all branches (even if Active branch selected); version names/descriptions excluded. Dropdown filters: **Tab** (default Any tab) · **Tab types** (Part Studio / Assembly / Drawing; Clear) · **Modified by** (Only me / Not me / Any — default Any) · **Date modified** (Start/End; default all) · **Search** · **Reset**.

### 3.3 Create version dialog [DOCS][BUNDLE]
- Entry points: Document panel Create version icon (stays in workspace); panel toolbar icon (for selected workspace); graph context menu **Create version here…** (on a history entry → "Create version from history"); shortcut ID `CREATE_VERSION` (no default key in docs).
- Fields: **Name** (prefilled next "Vn") · **Description** (multi-line).
- Warning block when drawings have pending updates (yellow Update icon): "These drawings should be updated before creating a new version" + Update icon that updates all listed drawings (stays updated even if dialog cancelled). [BUNDLE] "This drawing was not updated prior to version creation."
- Optional (when FeatureScript custom features exist): checkbox "Allow searches for published FeatureScript to find custom features and tables defined in this version" [BUNDLE].
- Buttons: **Create** (stay in current workspace) · **Create version and edit properties** (then opens version Properties dialog listing every tab/part with names/descriptions) · variants "Create version, publish, and edit properties", "Create version and update in assembly" (from Insert dialog of linked doc) [BUNDLE] · Cancel.
- Progress toast "Creating version…"; errors "Failed to create version.", "Cannot create a version with an undefined workspace."

### 3.4 Branch to create workspace [DOCS][BUNDLE]
- Version (or history entry) context menu > **Branch to create workspace…**.
- Dialog (Properties): **Name**, **Description**, checkbox **Open** (open the new workspace after creation).
- Buttons: **Create** · **Create workspace and edit properties**.
- New workspace content = the version; new branch drawn from that version node. "Branched from __versionName__".

### 3.5 Graph context menu — every item [DOCS][BUNDLE]
Items vary by target (change / version / branch / own branch):
- **Open** – open that change/version/branch in current window.
- **Open in new browser tab**.
- **Copy link** – URL to that point/branch/version.
- **Compare** – compare with current (§3.9).
- **Merge into current workspace…** – §3.8 (on another branch/version).
- **View in repair** – §3.13.
- **Restore to [Workspace]…** – §3.7 (on earlier point of same branch, or on a version).
- **Create version here…**.
- **Branch to create workspace…** (on versions).
- **Workspace protection…** – §3.10.
- **Properties** – branch/version Name, Description; Category and tab tree.
- **Publish FeatureScript** – if doc has custom features (out of scope beyond flag).
- **Delete** – delete branch's workspace (Main and versions cannot be deleted). Confirm dialog → Delete. If branch has no versions: branch + workspace removed; otherwise workspace removed and branch reverts to latest version (closed dot). Requires owner or Delete permission.
- **Scroll to branch top / bottom** (branch title).

### 3.6 Viewing history points and versions (read-only) [DOCS][BUNDLE]
- Click a history entry → document renders at that point (incl. the listed feature) in **view-only**; banner "History changes are view only. Viewing __momentName__", with **Return to <branch-name>** to go back to live workspace.
- Opening a version: "Versions are view only. Viewing __versionName__." Workspace name area shows version name; tab context menu reduced (§2.5); Document menu shows Copy version / Version properties.
- "Tab only exists in '__versionName__'." / "Can't open tab. The tab does not exist in this __documentWorkspaceType__." when switching to a point lacking the tab.

### 3.7 Restore [DOCS][BUNDLE]
- Target = active workspace (must open the workspace to restore it). Right-click earlier history point (Source, same branch) or a version > **Restore to B1…**.
- **Restore dialog**: header shows source moment (e.g. `Part Studio 1 :: Insert feature : Fillet 1`); "Loading restore preview…".
  - **Overall restore strategy**: **Keep [Target]** (no change) / **Replace with [Source]** (default; shows "*n changes*" link → opens Compare in new tab). Changing overall switches all changed tabs.
  - **Tab restore strategy** per tab (overrides): same two options. Row icons: green ✓ = edits kept in target; red ✗ = discarded edits (e.g., deleted features) kept; ⚠ = tab will not exist after; info = non-mergeable.
  - Tab order: tabs with changes first, then without; each in tab-bar order; folders hidden.
  - **Restore** button. Result is a new history entry at the top of the branch (restore is itself undoable via history).
- Tip: restore just some tabs → overall Keep, per-tab Replace.

### 3.8 Merge [DOCS][BUNDLE]
- Select (activate) the **target** workspace, right-click the **source** branch/version/change → **Merge into current workspace…**. Dialog title "Merge into: __target__, from: __source__".
- Semantics: delta from **base** (common ancestor = branch point version, or last non-reverted merge point of same source→target) to **source**, applied to **target**.
- **Overall merge strategy**: **Keep [Target]** · **Merge changes** (*default*; "Merge changes from both") · **Replace with [Source]**. Link **Compare with base** (opens compare in new tab).
- **Per-tab strategy** dropdown overriding overall; for created/deleted tabs and for non-mergeable tab types (**Drawings, PCB Studios**) only Keep / Replace ("For this tab type, edits from separate branches cannot be merged."). Same row icons as restore.
- No changes between source and target → only Keep/Replace, Replace default; if all Keep → **Merge** disabled.
- Granularity: whole tab only — cannot pick individual changes within a tab. Part Studio "Merge changes" merges at feature level.
- Blocking conditions (warnings in dialog): FeatureScript/feature-library version mismatch → **Update FeatureScript** icon → Update now ("Update the feature library in __workspace1__ and __workspace2__?"); document open in other windows/by other users (lists users/instances; auto-refresh; **Retry** on verification timeout); update scheduled/in progress; protected target out of date (must first merge protected → working branch).
- After merge: persistent message with **Revert to before this merge** link (survives tab/workspace switching); close (X) accepts. Merge **cannot be undone with Undo**; reverting loses later edits. Toast "Merged __source__ into __target__."
- Graph draws merge line from source to target.

### 3.9 Compare [DOCS]
- Entry: panel **Compare** icon then pick two entries (first highlighted **blue** = Base, second **red** = Target; expand Changes to pick specific entries), or context menu **Compare** (active workspace/version = Base).
- Available for Part Studios and Feature Studios ("Compare not supported for this tab." otherwise).
- Part Studio compare panel: side-by-side feature lists (Base left, Target right); difference icons: none = identical · ≠ not identical · < only in Base · > only in Target · ↑/↓ moved position. Toggle "show differences only" vs list all features. Graphic overlay: Base geometry blue, Target red, with a **slider** between Base and Target labels to blend. **Reverse compare** swaps sides. Tab selector to choose other Part Studios. Configuration selector when configured. Context menu Show/Hide Part. Tolerance differences outlined in yellow. [BUNDLE] "Compare has found differences." / "...no differences."
- Feature Studio compare: line-by-line diff, additions green, deletions red, Reverse compare.

### 3.10 Workspace protections [DOCS]
- Context menu on workspace > **Workspace protection…** → dialog checkbox **Enable workspace protection** → **Apply**. Lock icon in graph; banner "Workspace protection enabled. Changes must be merged in from another workspace."
- Blocked on protected workspace: geometry changes, drawing changes, certain metadata changes, creating/deleting tabs ("This operation is not permitted in a protected workspace."). Allowed: release-mgmt metadata, refresh drawings, version creation, branching, merging.
- Workflow: branch from a version → edit → merge into protected; if protected moved ahead, merge protected → branch first.

### 3.11 Undo / redo semantics [DOCS][BUNDLE]
- **Ctrl/Cmd+Z** undo, **Ctrl+Y / Shift+Cmd+Z** redo; repeatable. Toolbar Undo/Redo arrows at left of toolbar.
- Scope: "per user, per tab, per session" (UI basics) — undo only reverses *your own* changes made in the current session in the active tab; redo the last undone. Each undo/redo is itself recorded as a history entry ("Undoing change(s)", "Redoing change(s)" [BUNDLE]).
- Merge is not undoable via Undo (use revert link). Restore/history give unlimited recovery ("nothing in Onshape is ever truly deleted" — glossary).
- Clone: maintain per-user per-tab undo stack over the workspace change log; undo appends an inverse change rather than truncating log.

### 3.12 Linking documents, update notifications, Reference manager [DOCS]
- References across documents (or to another version of same doc) are **always to a version** (immutable) → using doc history stays reproducible. Created by: Assembly Insert from *Other documents*, Derived feature, Move tab to document, Variable Studio/Feature Studio imports.
- Insert dialog: source doc must have a version; else message with link to open **Create version** dialog for that doc ("Create version and update in assembly").
- Indicators next to linked instances: link icon (up-to-date); **blue background** = newer version on same branch with edits to that tab; triangle = revision; thumbtack = pinned; small arrow = child out of date; grey circle variants for pinned/children. Out-of-date propagates up to parents. Toolbar **Update all references** button active when unpinned updates exist. [BUNDLE] "Newer versions of linked documents available".
- **Reference manager** (click icon or Update all): modes **Update to latest** (recursive; auto-creates versions in referenced docs as needed; prompts for branch if newer versions on multiple branches; "Update to the new document" for moved instances; disabled if any doc lacks write permission) and **Selective update** (checkbox per reference, version-graph picker to choose any version incl. older, filters Pinned / Revision, **Update selected**).
- **Pin reference** (instance icon context menu) excludes from Update all. Context menu **Update linked document…**, **Open linked document** (opens source doc at the linked version in new tab).
- Errors: linked doc in trash / deleted / no permission ("You cannot modify this feature because you cannot access the referenced document.").
- Unsharing only blocks updating to newer versions; existing links keep working.

### 3.13 Where used [DOCS]
- From part/assembly context menu (Parts list, Instances list, graphics) or Feature Studio tab context menu; opens in Document panel. Results only from **versions** (workspaces unsupported: "Where used for workspaces is not supported. Pick a version in the filter options…").
- Version selector (part version/revision; "Show more" lists each part per version for multi-part studios; configurations dropdown). Result filter: Latest revision / All revisions / Latest versions / All versions. Results tree: assembly/drawing usages recursively with version, state, part number. Result context menu: Where used on that result, open in Isolate mode in new tab. Icons: Not permitted, Failed captures. (Pro/Enterprise only in Onshape; clone can implement locally across known projects.)

### 3.14 Repair (View in repair) [DOCS]
- From graph change context menu **View in repair** or panel Repair icon then pick a healthy change; or feature context menu **Edit healthy moment of [Feature]** / reference context menu **Edit healthy moment**.
- Opens **Repair panel** side-by-side (right) showing healthy state; synchronized view, rollback bar and configuration; hovering broken reference highlights where it pointed (yellow cross-highlight, incl. mate connectors). **Synchronize view** toggle. Repair panel context menu: Zoom to fit (f, double-click wheel), Isometric (Shift+7).
- **Replace reference** button at lower-right of a reference field (only with Repair panel open): pick new entity, **Propagate changes** checkbox (propagates replacement to downstream failing features). 1-to-1 only; not for mate connectors, in-context refs, external sketch refs, split references. Blue notice if chosen change isn't healthy. [BUNDLE] "Checked __n__ recent document history moments - no healthy state found."

---
## 4. Import / export

### 4.1 Supported formats [DOCS]

**Import — parts** (B-rep translated; meshes view/reference only):
| Format | Ext | Versions / notes |
|---|---|---|
| Parasolid B-rep (preferred) | .x_t, .x_b | v10.0 – v38.1 |
| Parasolid mesh | zip(x_t+xmm_txt) / zip(x_b+xmm_bin) / x_b | v28.0 – v37.0, mesh only |
| Parasolid mixed model (mesh + B-rep) | same zips / x_b | from v32.0 |
| ACIS | .sat | up to 2023 1.0 |
| STEP | .stp, .step | AP203, AP214, AP242 (geometry + face color only) |
| IGES | .igs, .iges | up to 5.3 |
| CATIA V5 | .CATPart | R7–R33 (V5-6R2024), V5-6R2026 |
| SOLIDWORKS | .sldprt | 1999 – 2026 |
| Inventor | .ipt | 9 – 2026 |
| Pro/ENGINEER / Creo | .prt | Pro/E 2000i – Creo Parametric 12.0 |
| JT | .jt | up to 11.0 |
| Rhino | .3dm | v2 – v8 |
| STL | .stl | mesh only |
| OBJ | .obj | mesh only |
| NX | .prt | UG15 – NX 2512 series (long list) |
| Solid Edge | .par, .psm | 10 – 2026 (.psm → not sheet metal) |
| glTF | .gltf | 2.0 |
| 3MF | .3mf | |
| PVZ | .pvz | Creo View |

**Import — assemblies**: Parasolid, ACIS, STEP, CATIA V5*, SOLIDWORKS Pack-and-Go zip*, Inventor .iam*, Pro/E/Creo*, JT, Rhino, NX*, Solid Edge*, glTF, 3MF. (* = must be a **zip** containing all parts + assembly, flat structure, zip name == top-level assembly name, no special chars, extension only `.zip`.) Message if assembly file uploaded without zip: "It looks like you want to import an assembly. Onshape needs a zip file containing the assembly and all of its references to process the import." [BUNDLE]

**Import — 2D drawings** (into Drawing / sketch *Insert DXF/DWG*): DWG ≤2018, DXF ≤2013/2018, DWT 2013/2018.

**Import — non-CAD** (stored as file tab; viewable): pdf, mp4, png, jpg/jpeg, svg, gif, txt, md, csv, json. Upload-only (not viewable): .py/.java/.js, .mov (browser-dependent), .doc/.docx. Any file type can be uploaded. Limit **4 GB per file**. Importing folders not supported [BUNDLE].

**Export — Parts / Part Studios**: Parasolid (.x_t/.x_b v25.0–v37.0), Parasolid mesh, Parasolid mixed, ACIS (.sat v5), STEP (AP203/214/242 incl. MBD data + face color), IGES 5.3, STL, JT ≤11.0, glTF, GLB, Rhino (.3dm v2–8), PVZ 8.0, 3MF, OBJ.
**Export — Assemblies**: Parasolid, Parasolid mixed, ACIS v5, STEP, IGES 5.3, PVZ 8.0, JT, glTF/GLB, OBJ, 3MF, **URDF** (assembly-only), STL.
**Export — Drawings**: PDF, DWG (R11-14, 2000, 2004, 2007, 2010, 2013, 2018), DXF (same), DWT template (2013, 2018), SVG, PNG, JPEG.
**Export — Sketches, planar faces, sheet-metal flat patterns**: DXF / DWG.

Format matrix (export, by source):
| Format | Part(s) | Part Studio | Assembly | Sketch/face | Drawing |
|---|---|---|---|---|---|
| Parasolid | ✓ | ✓ | ✓ | | |
| ACIS | ✓ | ✓ | ✓ | | |
| STEP | ✓ | ✓ | ✓ | | |
| IGES | ✓ | ✓ | ✓ | | |
| JT, PVZ, Rhino | ✓ | ✓ | ✓ (Rhino: parts) | | |
| glTF / GLB / OBJ / 3MF / STL | ✓ | ✓ | ✓ | | |
| URDF | | | ✓ | | |
| DXF / DWG | | | | ✓ | ✓ |
| DWT | | | | | ✓ |
| PDF / SVG / PNG / JPEG | | | | | ✓ |

### 4.2 Import options dialog ("Import to Onshape") [DOCS][BUNDLE]
Shown automatically when the file extension is recognized as CAD (else file is just stored as a tab). Initiated from Documents page (creates **new document** named after file, auto-creates a version; company users pick owner) or from inside a document via "+" > **Import** (creates new tabs named after file; "Import to this document").
- **Processing options (radio)**:
  - **Import to a single document** – Part Studios and Assemblies created as needed, named per source (small/medium assemblies, a few hundred parts).
  - **Split into multiple documents** – top assembly + subdocuments; >100 unique parts at a level → extra docs for Part Studios; checkbox *one document per part or Assembly* (docs placed in folder named after file). Top-level assembly gets Download only (no Update). Rules: parts-only file → each part own doc + generated top assembly linking them; assembly without subassemblies → single doc; with subassemblies → one doc per (sub)assembly. For large multi-level assemblies.
  - **Combine to a single Part Studio** – (flatten) assembly → one Part Studio of parts; duplicates for each instance.
- **Checkboxes**:
  - **Import appearances** (default ON) – face colors.
  - **Import material density** – SolidWorks, Solid Edge, CATIA V5, Pro/E, Creo, NX → custom material.
  - **Orient imported models with Y Axis Up** (default OFF; Onshape up = Z). Not for .ipt and .catpart.
  - **Create a composite part when importing multiple or non-solid bodies** – all parts → one closed composite part. Disabled when *Combine to a single Part Studio*.
  - **Join adjacent surfaces** – try to sew surfaces meeting at edges into parts (B-rep only).
  - **IGES postprocessing** (IGES only) – repairs linear edge translation (~50 % slower).
  - **Units** (STL/OBJ meshes): centimeter, foot, inch, meter, millimeter, yard (also changeable later from the imported tab's units dropdown).
  - Mobile also lists **Allow import of parts with faults** (desktop: always allowed; faulty parts shown with red names + notification).
- Auto-behavior: shattered body → surfaces/curves auto-grouped into one closed composite part.
- **Import** button. Progress list ("Importing __filename__…"); click file name to open created doc; notifications for success / "imported with errors" / failure with **retry**.

### 4.3 Import feature (in Part Studio) [FS][DOCS]
Feature list shows **Import N** (Derive-style icon); double-click/Edit opens dialog:
| Field | Type | Default | Visible |
|---|---|---|---|
| Import appearances | bool | true | always |
| Import material density | bool | false | only if source carries material data |
| Source is 'Y Axis Up' | bool | false | always |
| Allow faulty parts | bool | false | always |
| Unit | length unit | Meter | only for unit-less (mesh) sources |
| Create composite | bool | false | always |
Hidden params: blob/foreign id, flatten, maxAssembliesToCreate (10), composite name.
- **Update** (Import feature context menu, or **Update** button on imported file tab): pick new file; geometry replaced; downstream features re-applied where possible (errors otherwise). Only Part Studio updates — assemblies created by import are not updated.
- Mesh bodies listed under **Meshes** (below Surfaces) in Parts list; usable for: 3-point planes from mesh vertices, Measure (area, distances, mass props for closed meshes), sketch *Use* projection of mesh points, mate connectors at mesh points, *Up to vertex*. Mesh vertex picks are stored as positions (non-parametric).
- Repair tools for imports: **Highlight boundary edges** (View menu; laminar edges red solid/dashed hidden), Enclose, Fill, Move boundary, Bridging curve, Composite curve, Composite part, Delete face / direct edit tools.
- Imported file tab (blob): shows file info when not viewable; buttons **Update** and **Download**.

### 4.4 Export dialog — parts, Part Studios, instances, assemblies [DOCS][BUNDLE]
Entry: Parts list / Instances list / graphics context menu **Export…** (selected parts/instances; "Export selected…"), tab context menu **Export…**, Tab manager multi-select (all Part Studios or all Assemblies), Documents-page search result context menu. Field visibility by format:

| Field | Shown for | Options / default |
|---|---|---|
| **File name** | all | default = tab name (+ part name if single part). Chars `\ / * : \| > < ? "` → `-`. Date props → YYYY-MM-DD |
| **Format** | all | PARASOLID, ACIS, STEP, IGES, PVZ, JT, RHINO, GLTF, GLB, OBJ, 3MF, URDF (assembly only), STL |
| **Export models oriented Y axis up** | (3D formats) | checkbox, off |
| **Preprocessing** | STEP | None (default) · Automatic · Remove small entities only (advanced) · Remove small entities only (basic) · Remove small entities only (advanced) and convert all surfaces to b-surfaces |
| **Geometry format** | URDF | STL, GLTF, GLB, OBJ |
| **Use latest version** | PARASOLID, RHINO, STEP | checked (default) → Version locked |
| **Version** | PARASOLID, RHINO, STEP | enabled when *Use latest version* unchecked (Parasolid v25–37; STEP AP203/AP214/AP242; Rhino 2–8) |
| **Use custom units for export** | STEP | unchecked → meters; checked → Centimeter, Foot, Inch, Meter, Millimeter |
| **Include derived tolerances in export** | STEP | MBD |
| **Use custom annotation text height for MBD export** + *Annotations text height* | STEP | |
| **Use compatibility mode for surfaces and curves** | IGES | legacy CAM compat |
| **Resolution** | GLTF, GLB, OBJ, 3MF, STL, URDF | Coarse · Medium · Fine · Custom |
| Custom: **Distance tolerance**, **Angular tolerance**, **Maximum chord length** | GLTF/GLB/OBJ/3MF when Custom | |
| Custom: **Chordal tolerance**, **Angular deviation**, **Minimum facet width** | STL when Custom | |
| **Compress** | GLTF, GLB | compress meshes |
| **STL Format** | STL | Text · Binary |
| **Units** | STL | Centimeter, Foot, Inch, Meter, Millimeter, Yard |
| **Options** | all | Download · Download and store file in a new tab · Email with file download link · Store file in a new tab |
| **Export unique parts as individual files** | multi-part (STL, Parasolid…) | unchecked = one file; checked = zip of one file per part |
| **Apply export rules to contents** (+ *View export rules* link) | multiple tabs selected | checked default |
| **Include hidden instances in export** | assemblies | unchecked |
- Email option fields: recipients, subject, message, link expiry (default 3 days), *Require password for download* + password.
- Rules: suppressed parts/instances never exported; hidden states as set in doc ("__fileName__ will be exported with the hidden/shown states set in the document/version/protected workspace." [BUNDLE]); composite parts export as separate parts; OBJ export = zip of .obj (meters) + .mtl; exported data has no feature history. "MBD data is not exported for closed composite parts." "Selected tabs without any geometry were not exported." "The selected format or format version does not support the geometry type." [BUNDLE]
- **Export** button → background job, progress in Notifications ("__elementName__ is being prepared for export.").

### 4.5 Export sketch / planar face / flat pattern to DXF/DWG [DOCS]
Entry: sketch context menu **Export as DXF/DWG** (Feature list) or face/plane context menu in graphics ("Export DXF/DWG of __name__…"). Fields:
1. **File name**
2. **Format**: DXF / DWG
3. **Version** (R11-14 … 2018)
4. **Units** or checkbox **Use workspace units** (sketches export in document default units)
5. **Options** (download / store in tab …)
6. **Export splines as polylines** (default: splines stay splines)
7. **Set z-height to zero and normals to positive**
- [BUNDLE] also "Export inner edges" option for faces. Planar faces export outer solid geometry only (no dimensions/interior geometry).

### 4.6 Export Drawing dialog [DOCS][BUNDLE]
Entry: drawing tab context menu **Export…**; multiple via Tab manager > *Export drawings*. Fields vary by format:
| Field | Formats | Options |
|---|---|---|
| File name | all | |
| Format | | PDF, DWG, DXF, DWT, SVG, PNG, JPEG ("Selected format only supports exporting the active sheet." for image formats) |
| Version | DWG/DXF/DWT | R11-14…2018 |
| Sheets | all | All sheets · Custom sheets (range/numbers) |
| Overridden dimensions | all | Show underlines · Hide underlines |
| Text | PDF | Normal · Selectable |
| Text | DWG/DXF/DWT | Export as text · Explode into polylines (notes) · Explode all into polylines |
| Color | PDF | Color · Black and white · Grayscale |
| Units | DWG/DXF | Use drawing units · specify unit |
| Options | all | Download / Download and store in new tab / Email link / Store in new tab |
| Apply export rules to contents | multi-drawing | |
| Export splines as polylines | all but PDF | |
| Set z-height to zero and normals to positive | DWG/DWT/DXF | |
| Hide inspection items | all | inspection items shown by default |
| Exclude off-sheet content | DWG/DWT/DXF | |

### 4.7 Download [DOCS]
Tab context menu **Download** for any tab representable as a file (imported blobs, stored exports): copies original bytes with same name/type. Clone: native Save dialog.

**Clone notes**: implement STEP (AP214/242), Parasolid-equivalent native exchange (if kernel allows), IGES, STL (text/binary, units, resolution presets → chord/angle/min facet), OBJ(+MTL zip), 3MF, glTF/GLB, DXF/DWG for sketches/faces/drawings, PDF/SVG/PNG for drawings. "Store file in a new tab" = add exported bytes as a blob tab in the project.

---
## 5. Keyboard shortcuts

### 5.1 Shortcut map UI and customization [DOCS][BUNDLE]
- Open the **Keyboard shortcuts** map: Help (?) > *Keyboard shortcuts*, or **Shift+/** (toggle). Map has context tabs **Show All · General · Part Studio · Assembly · 3D View · Sketch · Feature Studio · Drawing** and a **Search shortcuts** field. (ID `OPEN_KEYBOARD_SHORTCUTS`.)
- Customization: Account menu > *My account* > *Preferences* > **Keyboard shortcuts** section. Customizable contexts: General tools, Part Studios, Assemblies, 3D view, Sketches, Feature Studios, Drawings.
  - Hover a row → pencil icon → **Edit shortcut** dialog: press a single key or combination.
  - Conflict messages: **Onshape-controlled shortcut conflict** (error; key reserved, cannot assign) · **Browser shortcut conflict** (info; key used by browser/OS) · **Override shortcut conflict(s)** (warning; already used by one/multiple other shortcuts — "This shortcut combination is already in use by __x__" [BUNDLE]).
  - Accept with green ✓ or **Enter**. **Disable shortcut** checkbox (shortcut removed/unusable); uncheck to re-enable. **Reset to default** (per shortcut; fails if default now conflicts: "Edit the combination for __x__ in order to reset this shortcut"). **Reset all** button top-right ("Reset shortcuts to defaults?" / "Are you sure you want to reset all keyboard shortcuts to default values?").
  - Customizations sync to all clients (web/iOS/Android).
- Static (non-editable) hotkeys listed in §5.10 are "Onshape-controlled".
- Notation: `ctrl/⌘` = Ctrl on Windows/Linux, Cmd on Mac; `alt/⌥` likewise. Where docs give only `ctrl` (e.g. Ctrl+Space, Ctrl+U, Ctrl+M, Ctrl+Q in drawings), Mac uses Ctrl as well per docs. `*` in Sketch table = marked in docs (likely core sketch tools; kept verbatim).

### 5.2 General [DOCS]
| Key (Win/Linux) | Mac | Action |
|---|---|---|
| Shift+Enter | Shift+Enter | Commit the command and open another dialog of the same command (accept & repeat) |
| Enter | Enter | Commit the command in use |
| Escape | Escape | Exit the command in use (also Render Studio) |
| Space | Space | Deselect selected feature(s)/clear selection (also Render Studio) |
| Ctrl+C | ⌘+C | Copy selected feature |
| Shift+C | Shift+C | Curve and surface analysis dialog |
| Delete / Backspace | Delete / Backspace | Remove selected features (also Render Studio) |
| Shift+D | Shift+D | Dihedral analysis |
| Ctrl+Shift+F | ⌘+Shift+F | FeatureScript search (find text in current Feature Studio) |
| Ctrl+U | Ctrl+U | Feedback / Report a bug |
| A | A | Flip primary axis (flips z-axis when editing, e.g. mate connector) |
| K | K | Show/hide Mate connectors |
| Shift+/ | Shift+/ | Toggle Keyboard shortcuts list |
| Shift (hold) | Shift (hold) | Lock mate inference |
| Ctrl+M | Ctrl+M | Mate connector dialog |
| [ | [ | Measure dialog |
| Ctrl+Click | ⌘+Click | Open selected document in a new browser tab |
| Shift+Click | Shift+Click | Open selected document in a new browser window |
| Ctrl+V | ⌘+V | Paste copied feature (repeatable) |
| Ctrl+Space | Ctrl+Space | Switch between recently opened tabs (session MRU; Part Studios, Assemblies, Drawings) |
| Ctrl+Y | Shift+⌘+Z | Redo (repeatable) |
| Shift+N | Shift+N | Rename selected instance or feature |
| Q | Q | Construction tool toggle (needs sketch plane; on preselected entities or while creating) |
| Alt+C | ⌥+C | Search tools bar (top right), focus cursor |
| ` (backquote) | ` | Select other: list of selectable entities under cursor (adds to current highlight) |
| S | S | Shortcut toolbar at cursor |
| Alt+T | ⌥+T | Tab manager (also Render Studio) |
| Ctrl+Z | ⌘+Z | Undo (repeatable; also Render Studio) |

### 5.3 Part Studio [DOCS]
| Key | Action |
|---|---|
| Shift+E | Extrude (uses open sketch; otherwise select face/sketch) |
| Shift+F | Fillet (edge fillet dialog) |
| Shift+H | Show/hide sketches |
| select rollback bar + ↑/↓ | Move rollback bar up/down through Feature list |
| Shift+W | Revolve (uses open sketch; otherwise select face/sketch) |
| Shift+S | New Sketch (prompts *Select a sketch plane*) |

### 5.4 Assembly [DOCS]
| Key | Action |
|---|---|
| J | Show/hide mates |
| I | Insert parts and assemblies dialog |
| M | Fasten mate dialog |
| H | Toggle *Show mates* mode on toolbar |
| Shift+S | Toggle *Snap* mode on toolbar |

### 5.5 3D View [DOCS] (most also in Render Studio)
| Key | Action |
|---|---|
| Shift+1 | Front view |
| Shift+2 | Back view |
| Shift+3 | Left view |
| Shift+4 | Right view |
| Shift+5 | Top view |
| Shift+6 | Bottom view |
| Shift+7 | Isometric view |
| Shift+P | Hide all construction geometry |
| Y | Hide selected or hovered instance(s) |
| Shift+Y | Show hidden instance(s) |
| P | Hide/show planes |
| Shift+I | Isolate selected/hovered instance(s) (others transparent) |
| Shift+T | Make selected/hovered instance(s) transparent |
| Shift+V | Named views dialog (prompt for new view name) |
| Ctrl+Shift+←→↑↓ | Pan design left/right/up/down |
| Ctrl+←→↑↓ | Rotate 5° (precision) |
| Shift+←→↑↓ | Rotate 90° |
| ←→↑↓ | Rotate 15° (docs call it "coarse rotation") |
| Shift+X | Section view dialog (multiple planes allowed) |
| Alt+Click (⌥+Click) | Select through transparent |
| Shift+R | View in high quality |
| N | View normal to preselected/hovered plane or face; press again → flip to inverse normal |
| Shift+Z | Zoom in (center of view) |
| Z | Zoom out |
| F | Zoom to fit |
| W | Zoom to window (drag box) |

### 5.6 Sketch [DOCS] (all require an active sketch plane)
| Key | Action |
|---|---|
| A | 3 point arc |
| C * | Center point circle |
| R * | Center point rectangle |
| I * | Coincident |
| Shift+O | Concentric |
| G * | Corner rectangle |
| Shift+U | Curvature |
| D * | Dimension |
| E | Equal |
| X | Extend |
| Shift+J | Fix |
| H * | Horizontal |
| L * | Line |
| Shift+A | Switch line ↔ tangent arc while drawing |
| Shift+M | Midpoint |
| Shift+K | Normal |
| O | Offset |
| B * | Parallel |
| Shift+L * | Perpendicular |
| Shift+G | Pierce |
| Shift+S | Sketch point |
| Shift+F | Sketch fillet |
| Shift (hold) | Suppress automatic inferences while moving/placing entities |
| Shift+Q | Symmetric |
| T * | Tangent |
| Q | Construction toggle |
| M * | Trim |
| U | Use (Project/Convert) |
| V * | Vertical |
Each key "selects or exits" the tool (toggle).

### 5.7 Feature Studio [DOCS]
| Key | Action |
|---|---|
| Ctrl/⌘+] | Navigation history: per docs "return cursor to its previous position" (Back reference) |
| Ctrl/⌘+[ | Navigation history: per docs "next position" (Forward reference) — docs pairing appears swapped vs. convention; IDs `BACK_REFERENCE`/`FORWARD_REFERENCE` |
| Ctrl/⌘+S | Commit current Feature Studio (updates other tabs) |
| Ctrl/⌘+Shift+S | Commit all Feature Studios with uncommitted changes |
| Escape | Dismiss autocomplete |
| Ctrl/⌘+Shift+O | Top level symbols (outline) dropdown |
| Ctrl/⌘+Shift+F | Text search pane |

### 5.8 Drawings [DOCS]
| Key | Action |
|---|---|
| Alt/⌥+LMB drag | Copy selected annotation(s) to drag location |
| Shift+D | Diameter dimension tool |
| Shift+H | Foreshorten dimension tool |
| Ctrl+S | Hide/show Sheets panel |
| Home | First sheet |
| End | Last sheet |
| Page Down | Next sheet |
| Page Up | Previous sheet |
| Ctrl+M | Maximum/minimum dimension tool |
| N | Note tool |
| P | Projected view tool |
| Shift+Q | Activate midpoint and quad points on arcs/circles during dimensioning |
| Shift+R | Radial dimension |
| Ctrl+Q | Update all views and properties on each sheet |

### 5.9 Additional shortcuts found only in the Glossary [DOCS]
| Key | Action |
|---|---|
| Ctrl/Cmd+U | Contact support (glossary; shortcuts page says Ctrl+U = Report a bug; ID `OPEN_CONTACT_SUPPORT`) |
| Q | Reorient secondary axis (mate connector dialog; ID `FLIP_SECONDARY_AXIS`) |
| Delete | Trash (Documents page: send selected doc to trash) |
| Ctrl/Cmd+Left click | Open context menu (Mac one-button) |
| Alt+T | "Search tabs" (= Tab manager) |
| F / double-click wheel | Zoom to fit (also Repair panel) |
| Space | Clear selection (context-menu label) |
| P (context menu, planes selected) | Hide all planes |
| N / Shift+X / ` / F | shown beside *View normal to* / *Section view* / *Select other* / *Zoom to fit* in context menus (context menus display shortcut hints) |

### 5.10 Static (Onshape-controlled, non-editable) hotkeys [BUNDLE]
| Action | Key |
|---|---|
| Pan | Ctrl+Shift+Arrows |
| Rotate | Arrows (15°) |
| Precision rotate | Ctrl+Arrows (5°) |
| Coarse rotate | Shift+Arrows (90°) |
| Select through transparent | Alt+Click |
| Copy annotation (drawings) | Alt+LMB drag |
| Accept command | Enter |
| Accept & repeat command | Shift+Enter |
| Lock mate inference | Shift (hold) |
| Open in new window | Shift+Click |
| Open in new tab | Ctrl/Cmd+Click |
| Move rollback bar | select rollback bar + Up/Down arrows |
| Suppress inferencing (sketch) | Shift (hold) |
(Note: glossary labels Shift+arrows "Precision rotate" — conflicting; the 5°/90°/15° step values from the shortcuts page are authoritative.)

### 5.11 Bindable command IDs (client bundle `bundle-command-actions.txt`) [BUNDLE]
Constant names (string values are camelCase equivalents, e.g. `OPEN_TAB_MANAGER=openTabManager`, `CREATE_FEATURE_REVOLVE=revolve`). Known default key in parentheses (from docs).

- **General / app**: OPEN_KEYBOARD_SHORTCUTS (Shift+/), UNDO_CHANGE (Ctrl/⌘+Z), REDO_CHANGE (Ctrl+Y / ⇧⌘Z), DELETE_SELECTION (Delete/Backspace), CLEAR_SELECTION (Space), SELECT_OTHER (`), RENAME_SELECTION (Shift+N), CANCEL (Esc), OPEN_COMMAND_SEARCH (Alt+C), OPEN_MINI_TOOLBAR (S), OPEN_CONTACT_SUPPORT (Ctrl+U), OPEN_TAB_MANAGER (Alt+T), OPEN_ELEMENT_TAB_SWITCHER (Ctrl+Space), COPY_OCCURRENCES (Ctrl/⌘+C), PASTE_OCCURRENCES (Ctrl/⌘+V), OPEN_DEBUG_PANEL, CREATE_VERSION, CREATE_RELEASE, EXPAND_ALL_FEATURE_FOLDERS, COLLAPSE_ALL_FEATURE_FOLDERS, ADD_CUSTOM_FEATURE.
- **Mate connector / axes**: CREATE_FEATURE_MATE_CONNECTOR (Ctrl+M), HIDE_SHOW_MATE_CONNECTOR (K), FLIP_PRIMARY_AXIS (A), FLIP_SECONDARY_AXIS (Q).
- **Analysis**: CURVE_SURFACE_ANALYSIS (Shift+C), DEVIATION_ANALYSIS, CONNECTION_ANALYSIS, DIHEDRAL_ANALYSIS (Shift+D), SURFACE_FLATTEN, SHOW_MEASURE_DIALOG ([), INTERFERENCE_DETECTION, ZEBRA_STRIPES, CURVATURE_COLOR_MAP, DRAFT_ANALYSIS, REFLECTION_ANALYSIS, MASS_PROPERTIES, OPEN_THICKNESS_ANALYSIS.
- **Part Studio features**: CREATE_FEATURE_SKETCH (Shift+S), CREATE_FEATURE_EXTRUDE (Shift+E), CREATE_FEATURE_FILLET (Shift+F), CREATE_FEATURE_REVOLVE (Shift+W), CREATE_FEATURE_SWEEP, _LOFT, _THICKEN, _ENCLOSE, _FACE_BLEND, _CHAMFER, _DRAFT, _BODY_DRAFT, _RIB, _SHELL, _HOLE, _EXTERNAL_THREAD, _LINEAR_PATTERN, _CIRCULAR_PATTERN, _CURVE_PATTERN, _MIRROR, _BOOLEAN, _SPLIT, _TRANSFORM, _WRAP, _DECAL, _DELETE_PART, _MODIFY_FILLET, _DELETE_FACE, _MOVE_FACE, _REPLACE_FACE, _PLANE, _OFFSET_SURFACE, _BOUNDARY_SURFACE, _FILL, _MOVE_BOUNDARY, _RULED_SURFACE, _MUTUAL_TRIM, _CONSTRAINED_SURFACE, _HELIX, _THREE_D_FIT_SPLINE, _PROJECTED_CURVE, _BRIDGING_CURVE, _COMPOSITE_CURVE, _INTERSECTION_CURVE, _TRIM_CURVE, _ISOCLINE, _OFFSET_CURVE, _ISOPARAMETRIC_CURVE, _EDIT_CURVE, _ROUTING_CURVE, _DERIVED, _VARIABLE, _QUERY_VARIABLE, _COMPOSITE_PART, _FRAME, _FRAME_TRIM, _GUSSET, _END_CAP, _CUT_LIST, _TAG_PROFILE, _SHEET_METAL_MODEL, _FLANGE, _HEM, _TAB, _BEND, _SHEET_METAL_JOG, _FORMED, _SHEET_METAL_LOFT, _MAKE_JOINT, _CORNER, _BEND_RELIEF, _FINISH_SHEET_METAL_MODEL, _CORNER_BREAK, _FLEX_PCB_MODEL, _FINISH_FLEX_PCB_MODEL (all prefixed CREATE_FEATURE_; no defaults unless listed).
- **Visibility**: HIDE_SHOW_SKETCHES (Shift+H), HIDE_SHOW_PLANES (P), HIDE_ALL_CONSTRUCTION (Shift+P), HIDE_SELECTED_PARTS (Y), SHOW_SELECTED_PARTS (Shift+Y), MAKE_TRANSPARENT (Shift+T), MAKE_ISOLATE (Shift+I).
- **Assembly**: MATE (M = Fastened), OPEN_INSERT_DIALOG (I), SNAP_MODE (Shift+S), SHOW_MATES_MODE (H), HIDE_SHOW_MATES (J), TANGENT_MATE, WIDTH_MATE, GROUP, REPLICATE, REPLACE_INSTANCE, ASSEMBLY_LINEAR_PATTERN, ASSEMBLY_CIRCULAR_PATTERN, ASSEMBLY_MIRROR_INSTANCE, MATE_RELATION, DISPLAY_STATES, CREATE_PART_STUDIO_IN_CONTEXT.
- **View / camera**: ZOOM_IN (Shift+Z), ZOOM_OUT (Z), ZOOM_TO_FIT (F), ZOOM_TO_WINDOW (W), FRONT_VIEW (Shift+1), BACK_VIEW (Shift+2), LEFT_VIEW (Shift+3), RIGHT_VIEW (Shift+4), TOP_VIEW (Shift+5), BOTTOM_VIEW (Shift+6), ISOMETRIC_VIEW (Shift+7), DIMETRIC, TRIMETRIC, VIEW_NORMAL_TO (N), NAMED_VIEW (Shift+V), SECTION_VIEW (Shift+X), VIEW_IN_HIGH_QUALITY (Shift+R), PREVIOUS_VIEW, PERSPECTIVE_VIEW, OPEN_GRAPHICS_PREFERENCES; render modes SHADED_WITH_EDGES, SHADED_WITHOUT_EDGES, UNSHADED, TRANSLUCENT, HIDDEN_EDGES_REMOVED, TANGENT_EDGES_VISIBLE, TANGENT_EDGES_PHANTOM, TANGENT_EDGES_HIDDEN, HIGHLIGHT_BOUNDARY_EDGES; ROTATE_LEFT/RIGHT/UP/DOWN (arrows), PRECISION_ROTATE_* (Ctrl+arrows), COARSE_ROTATE_* (Shift+arrows), PAN_ROTATE_* (Ctrl+Shift+arrows).
- **Sketch tools**: LINE (L), LINE_TANGENT_ARC_SWITCH (Shift+A), CORNER_RECTANGLE (G), CENTER_POINT_RECTANGLE (R), CENTER_POINT_CIRCLE (C), THREE_POINT_ARC (A), FILLET (Shift+F), TRIM (M), EXTEND (X), OFFSET (O), USE (U), DIMENSION (D), POINT (Shift+S), TOGGLE_CONSTRUCTION (Q), MIDPOINT_LINE, ALIGNED_RECTANGLE, THREE_POINT_CIRCLE, ELLIPSE, TANGENT_ARC, CENTER_POINT_ARC, ELLIPTICAL_ARC, CONIC, INSCRIBED_POLYGON, CIRCUMSCRIBED_POLYGON, SPLINE, BEZIER, SPLINE_CONTROL_POINT, TEXT, INTERSECTION, SPLIT, SLOT, MIRROR, LINEAR_PATTERN_SKETCH, CIRCULAR_PATTERN_SKETCH, TRANSFORM, INSERT_DXF_DWG, INSERT_IMAGE, CHAMFER, TOGGLE_SKETCH_PROFILE_INSPECTOR, TOGGLE_SKETCH_CONSTRAINT_MANAGER.
- **Sketch constraints**: COINCIDENT (I), CONCENTRIC (Shift+O), CURVATURE (Shift+U), CURVATURE_G3, EQUAL (E), FIX (Shift+J), HORIZONTAL (H), MIDPOINT (Shift+M), NORMAL (Shift+K), PARALLEL (B), PERPENDICULAR (Shift+L), PIERCE (Shift+G), SYMMETRIC (Shift+Q), TANGENT (T), VERTICAL (V).
- **Feature Studio**: TOGGLE_FEATURE_SCRIPT_SEARCH (Ctrl/⌘+Shift+F), COMMIT (Ctrl/⌘+S), COMMIT_ALL (Ctrl/⌘+Shift+S), DISMISS_AUTOCOMPLETE (Esc), BACK_REFERENCE, FORWARD_REFERENCE (Ctrl/⌘+] / [).
- **Drawing** tools are not in this ID list (drawing defaults per §5.8 must be defined separately).

Context-dependent reuse (clone must scope bindings by active context): Shift+S (Part Studio = Sketch, Sketch = Point, Assembly = Snap mode), Shift+F (Fillet vs Sketch fillet), A (3-pt arc vs Flip primary axis), Q (Construction vs Reorient secondary axis), M (Trim vs Fasten mate), H (Horizontal vs Show mates mode), I (Coincident vs Insert), P (planes vs Projected view), N (view normal vs Note), Shift+D (Dihedral vs Diameter dim), Shift+H (sketches vs Foreshorten), Shift+Q (Symmetric vs quad points), Shift+R (high quality vs Radial dim), Ctrl+M (Mate connector vs Max/min dim), Ctrl+S (Sheets vs Commit).

---

## 6. Collaboration (out of scope beyond comments) [DOCS]
Documents are shared (Share dialog: users/teams/companies, permissions Edit/Copy/Link/Export/Comment/Delete/Share; public/private; transfer ownership; folder-inherited, additive perms) and edited simultaneously; collaborator avatars ("social cues") appear in the toolbar, double-click one to **follow** (see their tab, view, selections, feature edits; click anywhere to stop); view-only users get a reduced View Only toolbar. **Comments** (Document panel): general or tagged to features, sketches, dimensions, MBD annotations, mate connectors (incl. implicit), mates, subassembly entities, via context menu *Add comment* or the *Tag entity* tool; @mentions, markup snapshots, URL auto-linking, replies, resolve/delete, assign-as-task; comment icons drawn on tagged entities only while the Comments panel is open. For the clone: implement only local, per-project comments with entity tags if desired.

## 7. Related tab types and libraries (one line each)
- **Feature Studio** [DOCS]: FeatureScript code tab; autosaves while typing, **Commit** (Ctrl/⌘+S) / **Commit all** (dropdown shows count of uncommitted studios) propagates to the document; Text/Symbol/Standard-library-import search pane (Match case, Regex, Feature Studio name filter, Replace all / Rename, Include/Exclude version ranges), Module outline (Ctrl/⌘+Shift+O), Where used via tab context menu, line-diff Compare.
- **Variable Studio** [BUNDLE]: tab for document-wide variables; option "Automatically insert this Variable Studio into all Part Studios and Assemblies in this workspace".
- **Material library** tab: custom materials ("Create Material library" in "+" menu).
- Library__onshape_material_library: Onshape-maintained material definitions applied via part Material.
- Library__default_tolerances_library / onshape_default_tolerances_library: MBD tolerance default sets (Part Studio).
- Library__onshape_frame_profiles_library: standard frame profile sketches for the Frame feature.
- Library__sheet_metal_form_library: standard sheet-metal Form library for the Form feature.
- Library__onshape_standard_content_library: fasteners etc. inserted via Assembly Insert > Standard content.
- Folder context menu *Set library* designates a folder as company/user frame, forms or images library.

## 8. Key implementation rules (summary)
1. Every persisted edit appends a history entry `Tab :: Action : Feature` to the active workspace; no Save command; history browsable/restorable at any entry; versions are immutable whole-document snapshots; branches fork at versions with ≤1 workspace each.
2. Restore and Merge are tab-granular with Keep/Replace(/Merge for non-drawing tabs) strategies, computed against a base (common ancestor or last merge); both produce new history entries; merge is reverted via a persistent link, not Undo.
3. Undo/redo is per user, per tab, per session and is itself recorded in history.
4. Cross-document references always pin a version; a blue-highlighted link icon signals newer versions; Update all recursively versions and updates; pinning excludes.
5. Tab order is persistent/shared; new tab inserted right of active and activated; tab context menu differs for versions (read-only subset).
6. Import creates a blob tab plus derived Part Studio/Assembly tabs with an editable **Import** feature (appearances, Y-up, faulty parts, composite, units for meshes); re-importing via Update re-applies downstream features.
7. Export dialog fields are format-dependent (table §4.4); filenames sanitized (`\/*:|<>?"` → `-`).
8. Shortcuts are context-scoped, user-rebindable (with reserved-key, OS-key and duplicate conflict checks, disable, reset, reset all); a fixed set (§5.10) is non-editable.
