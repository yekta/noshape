## 1. Document model

Scope: how Onshape organizes CAD data (documents, tabs/elements, workspaces, versions, branches, history), how edits are persisted and undone, how documents reference each other (linking, derived, reference updates), how units and numeric expressions work, plus a proposed data model for noshape. Cloud-only aspects (sharing, permissions, public documents, enterprise PDM/release) are covered briefly and marked P2 because a native clone will at most emulate them locally.

Conventions in this section:
- Help URLs are abbreviated `H:Area/page` = `https://cad.onshape.com/help/Content/Area/page.htm`.
- "Element" = Onshape's internal name for a tab (API and FeatureScript use "element"; help uses "tab"; glossary entry "Insert new element").
- The term "microversion" does not appear in the help corpus; it is Onshape REST-API terminology (each persisted change = one immutable microversion). Statements relying on it are marked (inferred).
- For non-geometry items the **OCCT mapping** field gives the implementation approach instead.

### 1.1 Documents and the Documents page

### DOC-01 Document (container concept)
- **Purpose:** Top-level unit of CAD data. Replaces the file concept: one document holds any number of tabs (Part Studios, Assemblies, Drawings, Variable Studios, Feature Studios, imported files) with complete parametric history and version graph.
- **Sources:** H:Document/documents, H:Document/document_management, H:Home/glossary ("Document").
- **Selection inputs:** none.
- **Parameters:** document-level attributes:

| Attribute | UI label | Type | Notes |
|---|---|---|---|
| name | Document name (bold in toolbar) | string | Editable inline by clicking name in Document toolbar, or Document menu > Rename document |
| description | Document details > Description | string | Shown in Details panel on Documents page |
| category | Document details > Category | enum (company-defined) | Professional/Enterprise only |
| notRevisionManaged | Not revision managed | bool | Only when Release management enabled (P2) |
| owner | Owner | user / company / enterprise | Set at creation; transferable |
| thumbnail | (Select as document thumbnail) | tab ref | Defaults to last opened Part Studio |
| location | folder | folder ref | See DOC-05 |
| notes | Document notes | markdown | See DOC-13 |

- **Behavior / semantics:**
  - Contents: parts, assemblies, drawings, imported data, images, "any kind of data"; each item lives in its own tab. Onshape does not dictate how many parts/assemblies go in a document.
  - All work in a document is done with complete parametric history; every action is recorded (DOC-30).
  - On creation a document gets: one empty Part Studio tab ("Part Studio 1"), one empty Assembly tab ("Assembly 1"), one version named "Start" and one workspace named "Main" (both renamable). The Start version contains the empty Part Studio and Assembly.
  - Opening a document activates the last tab the user accessed (per user); first open activates the first tab.
  - Ownership: individual accounts own their documents; in Professional company / Enterprise accounts the company/enterprise always owns documents; users in several companies choose the owner at creation. Owner has every permission incl. transfer ownership; creators in company accounts have all permissions except transfer.
  - Recommended practice (help): one document per design project; reusable items (standard parts) in their own documents and linked in; split documents approaching ~100 tabs for performance and link them.
- **Edge cases & errors:**
  - A document must always contain at least one tab (last tab cannot be deleted; Move to document is blocked if it would leave the source empty).
  - Opening a trashed document: "Failed to load document for workspace. Cannot open a document in the trash. Restore the document from Trash."
- **OCCT mapping:** N/A. App data model: Document = root object of a content-addressed store (DOC-66).
- **Priority:** P0

### DOC-02 Create document
- **Purpose:** Make a new empty document.
- **Sources:** H:Document/create_menu, H:Document/documents_page. URL shortcut `cad.onshape.com/new`.
- **Selection inputs:** none.
- **Parameters:**

| Field | UI label | Type | Default / notes |
|---|---|---|---|
| name | Name | string | "Untitled document" (inferred) |
| labels | Labels | multi-select of user labels | optional |
| location | Document location | folder | current folder if navigated into one |
| owner | Owner | user/company | only for multi-company users |

- **Behavior / semantics:**
  - Create > Document on the Documents page; creates Start version + Main workspace + Part Studio 1 + Assembly 1 (DOC-01).
  - Workspace units initialized from the user's account preference units (DOC-54).
  - Other Create menu entries: Folder, Publication, Import files, Import from (cloud drives), Project (enterprise), Label.
  - Importing a file from the Documents page also creates a new document named after the file, with a version created automatically (DOC-65).
- **Edge cases & errors:** Create button inactive when an enterprise user has read-only access to the current folder.
- **OCCT mapping:** N/A. App: "New document" command creating a store with an initial microversion containing two empty elements, plus a Start version pointing to it.
- **Priority:** P0

### DOC-03 Ownership, sharing, permissions, public/private (brief)
- **Purpose:** Access control for collaboration. Cloud feature; noshape may emulate with local files plus optional sync.
- **Sources:** H:Collaboration/share_documents, H:Collaboration/sharing_and_collaboration, H:Collaboration/transfer_ownership, H:Document/documents.
- **Selection inputs:** none.
- **Parameters (Share dialog):**

| Field | UI label | Values / notes |
|---|---|---|
| target | Individuals / Teams / Companies / Connections / Public / Link sharing | tabs vary by plan |
| level | Can edit / Can view | Edit = edit, copy, link, export, share, comment; View = view, copy, link, export, comment |
| flags | Copy, Link document, Export, Share, Comment, Delete | Link document requires Export; Delete allows deleting document or a branch workspace |
| link | Turn on link sharing / Allow exporting from the link | view-only anonymous link; link captures workspace/version, tab, configuration state |
| support | Share with Onshape support | toggle |

- **Behavior / semantics:**
  - Visibility states: Private (owner only), Shared, Public (all users can view and copy, not edit; Free plan documents are always public; not available in Enterprise).
  - Folder permissions are inherited by contents and are additive with per-document permissions.
  - To let others link to a document they need at least Read/Copy/Export plus Link document permission. Unsharing only blocks updating to newer versions and creating new links; existing links keep working.
  - Public document toolbar shows counters: Copies, Links, Likes.
  - All permission levels can be in the same workspace simultaneously; view-only users see edits in real time.
- **Edge cases & errors:** Free-account users shared into private documents can only view.
- **OCCT mapping:** N/A. App: optional; for a native app model as file-system permissions or a sync-server ACL.
- **Priority:** P2

### DOC-04 Documents page (document browser)
- **Purpose:** Home screen listing documents, folders, publications; entry point for create/open/organize.
- **Sources:** H:Document/documents_page, H:Document/panels, H:Document/list_view.
- **Selection inputs:** click selects row/tile (blue); clicking the underlined name opens; Ctrl+click multi-select; Shift+click or Shift+arrow range; arrow keys navigate; Ctrl/Cmd+click opens in new browser tab, Shift+click in new window.
- **Parameters:** layout state persisted per user: filter, view mode, folder grid/list, splitter positions, Details panel width (min 340 px), added columns.
- **Behavior / semantics:**
  - Regions: Search field (searches within active filter); Create button; left filter panel (DOC-07), integrations, Trash; "Last opened by me" thumbnail strip (company filter); Folders and Projects section (grid or list, sortable, collapsible, resizable divider); Documents and Publications section.
  - Views: Document view (default flat list/tiles; Type = Show all / Documents / Publications), List view and Structure view (Professional/Enterprise; released items, DOC-52).
  - Columns: Name, Modified, Modified by, plus Part number, Revision, State when present in search results; Name and Modified sortable; Add button opens column chooser (check to add, reorder, search).
  - Right side panels for selection: Details, Analytics (document dashboard), Share details, Properties, Versions and history (full version-graph actions without opening the document), Document notes. Clicking document name in a panel opens it in a new browser tab; hover shows Copy link.
  - Row commands: Share, Send to trash (Delete key), Labels.
  - Context menu (varies by filter, type, multi-selection, permissions): Open, Open in new browser tab, Rename, Copy workspace, Details, Versions and history, Labels, Share, Move to, Go to location (released items), Transfer ownership, Unshare with me, Make public, Make personal copy (max 10 docs/request, 100 requests/day), Send to trash, Remove from recently opened.
  - Drag documents onto folders (Document view only) or onto labels/Trash.
  - Thumbnail = last opened Part Studio unless set via tab context menu "Select as document thumbnail" (requires edit permission); reverts to most recently edited tab if that tab is deleted/moved.
  - Notifications bell: comments, upload/import status, export status, workspace out-of-date status; auto-deleted after 14 days.
- **Edge cases & errors:** Structure view not retained after leaving the top-level company filter.
- **OCCT mapping:** N/A. App UI: GPUI list/grid over a local library index (SQLite or similar) of document stores; thumbnails cached PNGs.
- **Priority:** P1

### DOC-05 Folders
- **Purpose:** Physical grouping of documents/publications/sub-folders; carries inherited share permissions.
- **Sources:** H:Document/folders.
- **Selection inputs:** folder tile/row; breadcrumbs.
- **Parameters:** name; parent folder; share entries; library role ("Set library": frames, sheet metal forms, images library).
- **Behavior / semantics:**
  - Create > Folder (optionally into an existing folder); nesting by creating inside an opened folder or drag-drop (needs permission on both).
  - Folder context menu: Open, Open in a new browser tab, Rename (also by clicking breadcrumb), Details, Share, Unshare with me, Move to, Transfer ownership, Send to trash (contents go to trash too), Set library.
  - Move to dialog: navigate folders, New folder, Sort, Show details (list of items to move), Move here. Moving into a folder requires Edit on folder and all permissions on the document. Moving between root folders transfers ownership to the destination root owner and re-inherits sharing.
  - Copy workspace of a document in a folder: copy placed in same folder if user has edit permission on it, else at account root.
  - Share dialog for folder has Direct and Effective tabs.
- **Edge cases & errors:** Drag-drop into folders only in Document view.
- **OCCT mapping:** N/A. App: map to file-system directories or library index tree.
- **Priority:** P1

### DOC-06 Labels
- **Purpose:** User-private tags on documents/publications for filtering.
- **Sources:** H:Document/labels.
- **Selection inputs:** one or more documents/publications (not folders).
- **Parameters:** label name.
- **Behavior / semantics:** Create via Create > Label, the "+" next to Labels in filter list, or in Details panel; apply via context menu Labels (checkbox list with search), drag-drop onto label, or at document creation; multiple labels per document; selecting a label in the filter list filters the list; Delete label removes it from all documents only. Labels are visible only to the user who created them.
- **Edge cases & errors:** Labels cannot be applied to folders.
- **OCCT mapping:** N/A. App: tag table in library index.
- **Priority:** P2

### DOC-07 Built-in filters
- **Purpose:** Predefined document list scopes.
- **Sources:** H:Document/filters, H:Document/trash.
- **Selection inputs:** filter entry in left panel.
- **Parameters:** n/a.
- **Behavior / semantics:** First filter depends on plan: "Owned by me" (free/standard), company name (professional), enterprise name (enterprise) = everything accessible. Then Recently opened (remove via context menu), Created by me, Shared with me, Teams (collapsed group), Labels (collapsed group), Public (columns Name, Modified, Owned by, Copies, Links, Likes), Trash. Search always runs within the active filter.
- **Edge cases & errors:** Enterprise admins may hide Public.
- **OCCT mapping:** N/A. App: saved queries over library index.
- **Priority:** P2

### DOC-08 Search and advanced search
- **Purpose:** Find documents and objects inside documents (parts, Part Studios, assemblies, drawings) by name/properties.
- **Sources:** H:Document/advanced_search, H:Document/documents_page.
- **Selection inputs:** none.
- **Parameters (advanced search dropdown):**

| Field | Values |
|---|---|
| Type | Document, Part, Part Studio, Assembly, Drawing, Feature Studio, Variable Studio, Project, Folder, Publication, All |
| Category | multi-select of element categories |
| Name, Description | text; `*` wildcard segments (e.g. `*600*`) |
| Part number, State, Revision | for parts/assemblies/drawings |
| Add criteria | Owner, Creator, Vendor, dates, Not revision managed (True/False), custom properties |
| Results from | Workspaces / Workspaces and Versions / Versions; latest only or all |
| Subfolders | include or exclude |

- **Behavior / semantics:** Partial and multi-word matching including punctuation; results show type icon, name, workspace/version icon and name, part number, state; sortable by Name/Modified; query reflected in URL (bookmarkable); Reset clears. Same search engine is used inside Insert and Derived dialogs ("Other documents").
- **Edge cases & errors:** Searches only within active filter/folder.
- **OCCT mapping:** N/A. App: full-text index (e.g. tantivy) over document/element/part metadata per version.
- **Priority:** P2

### DOC-09 Trash, restore, permanent delete
- **Purpose:** Soft delete of documents, folders, publications.
- **Sources:** H:Document/trash, H:Document/documents_page, H:Document/folders.
- **Selection inputs:** one or more items.
- **Parameters:** retention: 30 days (non-enterprise), 90 days (enterprise).
- **Behavior / semantics:** Send to trash via Delete key, trash icon, context menu, or drag onto Trash filter (confirmation dialog). Trash filter shows only items the user trashed. Per item: Restore or Delete (permanent); Empty trash deletes all permanently. Only owners (or permitted users) can delete; non-owners can "Unshare with me" instead.
- **Edge cases & errors:** Linked document in trash cannot be opened (error in DOC-01); permanently deleted linked document: "Failed to load document for version. Resource does not exist, or you do not have permission to access it." Existing linked references in other documents then cannot be opened (inferred: geometry already cached in the referencing document continues to display until regeneration needs the source).
- **OCCT mapping:** N/A. App: move store to a trash directory with timestamp; purge job.
- **Priority:** P1

### DOC-10 Copy workspace / Copy version (copy document)
- **Purpose:** Create a new, unlinked document from the current workspace or an open version.
- **Sources:** H:Document/document_menu, H:Document/documents_page, H:Collaboration/share_documents ("Make a copy to edit" for public docs).
- **Selection inputs:** none.
- **Parameters:** new document name; destination folder (with New folder, Sort).
- **Behavior / semantics:**
  - Copy workspace: new document containing a copy of the workspace state; original and copy are not linked in any way; the copy opens on the same tab. From the Documents page, Copy workspace copies the document's default workspace.
  - Copy version: available when a version is open; no release metadata is copied except part numbers.
  - History of the source is not carried over (inferred; the copy starts a new history with its own Start version/Main workspace).
  - External references (links to other documents) are preserved as the same version references (inferred).
  - Public documents show "Make a copy to edit".
- **Edge cases & errors:** Requires Copy permission.
- **OCCT mapping:** N/A. App: snapshot the element trees of a microversion into a fresh store; new document id; new element ids or preserved element ids (design choice: preserve ids so internal references stay valid).
- **Priority:** P1

### DOC-11 Document toolbar and Document menu
- **Purpose:** Document-level commands available inside an open document.
- **Sources:** H:Document/document_toolbar, H:Document/document_menu, H:Home/user_interface_basics.
- **Selection inputs:** none.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Toolbar (left to right): logo (back to Documents page), Document menu (hamburger), document name (bold, inline-rename), workspace or version name (gray), Copy link (URL includes active configuration, render mode, exploded view, panel states, view orientation), folder link, FeatureScript notices, Notifications, App Store, Learning Center, Share, Help menu, user menu. Public documents add Public badge, Copies, Links, Likes.
  - Document menu (workspace open): Rename document, Move to (folder), Document details (description, category, Not revision managed), Restore deleted workspaces (DOC-33), Copy workspace, Update workspace (DOC-41), Workspace units (DOC-54), Workspace properties (DOC-12), Switch to view only mode (DOC-64), Print (graphics area of active tab), Close document.
  - Document menu (version open): Copy version replaces Copy workspace; Version properties replaces Workspace properties; editing commands unavailable.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** N/A. App UI.
- **Priority:** P1

### DOC-12 Properties and metadata (document, workspace/version, tab, part)
- **Purpose:** Attach name/description/part number/etc. to documents, workspaces, versions, tabs and parts.
- **Sources:** H:Home/properties_metadata, H:Document/document_management ("Properties for workspace, versions and releases"), H:PartStudio/part_studios (Properties dialog), H:Document/versions_and_history.
- **Selection inputs:** part (Parts list context menu), tab (tab context menu), workspace/version/branch (Versions and history context menu), document (Documents page Properties panel).
- **Parameters (default part property fields, from H:PartStudio/part_studios):**

| Field | Type | Notes |
|---|---|---|
| Part list name | read-only string | mirrors Name |
| Name | string | |
| Description | string | |
| Category | enum | Professional/Enterprise editable |
| Part number | string | manual or generated ("Generate next part number") |
| Revision | string | manual or by release |
| State | enum In progress / Pending / Rejected / Released | editable only in a version with release management off |
| Vendor, Project, Product line | string | |
| Material | read-only | Unassigned if none |
| Title 1/2/3 | string | used by drawing title block |
| Not revision managed | bool | |
| Exclude from all BOMs | bool | |
| Unit of measure | enum | default Inch (BOM quantity unit) |
| Mass | computed / override | requires material |

- **Behavior / semantics:**
  - Workspace/Version properties dialog lists all tabs and the parts within them with names, descriptions, part numbers, revisions, states; branch Properties shows Name, Description, Category and the tab tree, each tab editable.
  - Versions are immutable for geometry and part names at that version, but version metadata (name, description, properties) can still be edited ("Create version and edit properties").
  - Dialog buttons: Apply, Save (store without applying), Reset all (also clears appearance, material, tessellation overrides so computed properties take effect), Undo/Redo within dialog.
  - Custom properties defined by company admins appear at the bottom; grayed fields are admin-controlled; configured properties exist (see configurations section).
  - Metadata overrides for public/standard content parts are per-user/company and carry forward to new versions.
- **Edge cases & errors:** Reset all has no effect on configured properties.
- **OCCT mapping:** N/A. App: property map per (element, part id) stored in element state; version metadata stored on version object (mutable side table, not part of the immutable snapshot hash).
- **Priority:** P1

### DOC-13 Document notes
- **Purpose:** Free-form Markdown notes attached to a document.
- **Sources:** H:Document/document_notes.
- **Selection inputs:** none.
- **Parameters:** markdown text.
- **Behavior / semantics:** Document panel > Document notes: Refresh, Edit notes (editor with Apply, Cancel, Preview, Close); standard Markdown (setext/ATX headings, blockquotes incl. nested elements, ordered/unordered lists, links). Also shown in Documents page Document notes panel.
- **Edge cases & errors:** Not stated whether notes are versioned (inferred: document-level, not part of workspace history).
- **OCCT mapping:** N/A. App: markdown blob on document; render with a Markdown crate.
- **Priority:** P2

### DOC-14 Document panel (left vertical panel)
- **Purpose:** Access to document-wide tools for all tab types.
- **Sources:** H:Document/document_panel.
- **Selection inputs:** none.
- **Parameters:** n/a.
- **Behavior / semantics:** Icons: Versions and history (DOC-34), Create version (DOC-32), Release drafts (P2), Comments (DOC-62), Document notes (DOC-13), Where used (DOC-50), Performance, AI Advisor, Explore Onshape, Action items, Tab manager (DOC-25). Panels open to the left of the Feature list / Instances list, pushing it right.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App UI dock.
- **Priority:** P1

### 1.2 Tabs (elements)

### DOC-15 Tab (element) types
- **Purpose:** Each tab is a typed element with its own data model and editor.
- **Sources:** H:Document/document_tabs, H:Home/glossary ("Tab (general)", "Document"), H:Document/importing_files, H:VariableStudio/variable_studio, H:FeatureStudio/feature_studios.
- **Selection inputs:** none.
- **Parameters:** element types:

| Type | Content model | Merge support | noshape priority |
|---|---|---|---|
| Part Studio | Ordered feature list (parametric history) producing parts, surfaces, curves, sketches, mate connectors, meshes; configurations; variable table; part properties | Merge changes (feature-level) | P0 |
| Assembly | Instances (refs to Part Studio parts/whole Part Studios/assemblies, by element id + workspace/version), mates, groups, mate connectors, configurations, exploded views, named positions | Merge changes | P0 |
| Drawing | Sheets, views referencing a Part Studio/Assembly (workspace or version), annotations | Keep/Replace only | P1 |
| Variable Studio | Table of variables (name, type, value expression, description), optional configurations, "publish to workspace" checkbox | Merge changes (inferred: "available for all tab types except Drawings and PCB Studios") | P1 |
| Feature Studio | FeatureScript source module; defines custom features/tables; imports other Feature Studios by version | Merge changes; Compare is line diff | P2 |
| File / blob tab | Arbitrary uploaded file (CAD source file, PDF, image, CSV, JSON, text, DXF/DWG, etc.); displays file info or a viewer | Keep/Replace (inferred) | P1 |
| Folder (tab folder) | Container for tabs, nestable | n/a (folders not shown in merge dialog) | P1 |
| Application / Render Studio / CAM Studio / PCB Studio / Material library | Out of scope (non-CAD) | - | - |

- **Behavior / semantics:**
  - Only one tab is active per browser tab/window; any tab can be opened in a new window to get several active tabs.
  - Tabs of type Part Studio/Assembly/Drawing/File get type icons and are filterable in the tab manager; Released filter appears when a version has released objects.
  - CAD file import produces a File tab (the original blob) plus generated Part Studio(s)/Assembly tab(s); the Part Studio contains an `importForeign` ("Import") feature referencing the blob (FS param `blobData` of type CADImportData when `dependsOnBlob`, else hidden `foreignId`).
  - Other element-reference parameter types in FeatureScript (tabReferences.fs): `PartStudioData` (buildFunction, configuration, partQuery, configurationData), `ImageData` (imageWidth, imageHeight, mediaType), `TableData` (csvData), `JSONData` (jsonData), `TextData` (textData), `CADImportData`. These are how features reference other tabs (incl. blob tabs) in the same or other documents.
- **Edge cases & errors:** "Assemblies cannot insert other assemblies as a version reference if they have the same element ID" (H:Assembly/insert_parts_and_assemblies) — i.e. element ids are stable across versions and self-reference by id is forbidden.
- **OCCT mapping:** N/A. App: `enum ElementKind { PartStudio, Assembly, Drawing, VariableStudio, FeatureStudio, Blob, ... }` each with its own serialized state; see DOC-67.
- **Priority:** P0

### DOC-16 Tab bar and active-tab semantics
- **Purpose:** Navigation among tabs at the bottom of the window.
- **Sources:** H:Document/document_tabs, H:Home/keyboard_shortcuts_and_hotkeys, H:Home/glossary ("Tab switching", "Tab bar").
- **Selection inputs:** click tab to activate; right-click for context menu.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Tab order is shared by all users of a workspace and persistent (part of workspace state, recorded in history).
  - Active tab and tab-bar scroll position are per user, not shared, not persistent across users; reopening a workspace restores that user's previously active tab (first tab on first open).
  - New tabs are inserted immediately to the right of the active tab and become active.
  - The active tab is always scrolled into view; active tab highlighted with blue border.
  - Hovering a tab shows a thumbnail of its contents.
  - Ctrl+Space: hold Ctrl, tap Space repeatedly to cycle previously opened tabs (thumbnail strip); release to open.
  - Switching tabs commits any open feature dialog (mobile doc states "When you switch tabs, any open feature will be committed").
  - Social cues on tabs show which collaborators are in which tab (DOC-63).
  - Out-of-date link icon is shown on a tab when it contains references with newer versions (DOC-46).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** N/A. App UI; tab order stored in workspace state, active tab in per-user local session state.
- **Priority:** P0

### DOC-17 Create tab (Insert new tab menu)
- **Purpose:** Add a new element to the document.
- **Sources:** H:Document/document_tabs, H:PartStudio/part_studios.
- **Selection inputs:** none.
- **Parameters:** menu entries (CAD-relevant): Create Part Studio, Create Assembly, Create Drawing, Create Variable Studio, Create Feature Studio, Create folder, Import, Paste tab (when a tab is on the clipboard). Out of scope: Add application, Create Material library, Render Studio, PCB Studio, CAM Studio.
- **Behavior / semantics:** "+" icon at bottom-left of the tab bar. New element gets a default name `<Type> <n>` ("Part Studio 2", "Assembly 2", "Drawing 1"...; inferred numbering = next unused integer). Create Drawing opens a template/format dialog (drawing section). Recorded as a history entry (e.g. "Insert element") (inferred wording).
- **Edge cases & errors:** Not allowed in protected workspaces (DOC-40) or versions.
- **OCCT mapping:** N/A. App command producing a new microversion.
- **Priority:** P0

### DOC-18 Rename tab
- **Purpose:** Change element name.
- **Sources:** H:Document/document_tabs, H:PartStudio/part_studios.
- **Selection inputs:** tab (context menu > Rename; or double-click name, inferred).
- **Parameters:** name (string).
- **Behavior / semantics:** Workspace only. Name is metadata of the element; references from other tabs/documents are by element id so renaming does not break links (inferred from id-based references). Recorded in history.
- **Edge cases & errors:** Names need not be unique (inferred; Onshape allows duplicate tab names).
- **OCCT mapping:** N/A.
- **Priority:** P0

### DOC-19 Duplicate tab
- **Purpose:** Copy an element within the same document.
- **Sources:** H:Document/document_tabs, H:PartStudio/part_studios.
- **Selection inputs:** tab.
- **Parameters:** none.
- **Behavior / semantics:** Creates an independent copy (not associative in any way) placed in the same document (next to the original, inferred). Default name "<name> (copy)" or similar (inferred). Duplicated drawing still references the same model element.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App: deep-copy element state with a new element id; internal entity ids (feature ids) may be kept since they are element-scoped.
- **Priority:** P0

### DOC-20 Copy to clipboard / Paste tab into another document
- **Purpose:** Copy an element between documents.
- **Sources:** H:Document/document_tabs, H:PartStudio/part_studios.
- **Selection inputs:** tab (context menu > Copy to clipboard).
- **Parameters:** none.
- **Behavior / semantics:** Copies the tab to an application clipboard; in the target document "+" > Paste tab inserts an unlinked copy. Works for Part Studios explicitly (help) and generally for tabs (H:Document/document_tabs "Copy the tab to the clipboard").
- **Edge cases & errors:** References inside the pasted tab to other tabs of the source document become cross-document references or break (not documented; inferred: references to other elements in the source document must be rewritten as version references or left broken). noshape should define this explicitly: rewrite intra-document refs to external refs pinned to a source microversion/version.
- **OCCT mapping:** N/A. App clipboard payload = serialized element + source document id + source microversion.
- **Priority:** P1

### DOC-21 Move to document
- **Purpose:** Move one or more tabs (or a tab folder) to a new or existing document, leaving links behind.
- **Sources:** H:Document/document_tabs, H:PartStudio/part_studios, H:Assembly/updating_references ("Update to the new document").
- **Selection inputs:** one or more tabs (Ctrl+click in tab manager) or a folder.
- **Parameters:**

| Field | UI label | Default / notes |
|---|---|---|
| target | New document / existing document | new document is created during the operation |
| moveReferenced | Move all referenced tabs | checked; for Assembly tabs moves the Part Studios its instances reference |

- **Behavior / semantics:**
  - Moved entity is removed from the source document entirely; every remaining reference to it in the source becomes a link (version reference) to the destination document (link icons in Feature list/Instances list).
  - Folders move with contents.
  - Drawing that references the workspace must first be changed to a version reference ("Change to version") before moving the model without the drawing.
  - Assembly instances moved to another document can be redirected with Reference manager "Update to the new document".
- **Edge cases & errors:** Blocked if it would leave the source with no tabs (red message). Moving to a shared document needs Edit and Link; moving from a shared folder needs Edit and Copy.
- **OCCT mapping:** N/A. App: two-store transaction: create version in destination containing the element, rewrite source references to (destDoc, version, elementId), remove element from source.
- **Priority:** P1

### DOC-22 Delete tab
- **Purpose:** Remove an element from the workspace.
- **Sources:** H:Document/document_tabs, H:PartStudio/part_studios.
- **Selection inputs:** one or more tabs.
- **Parameters:** none.
- **Behavior / semantics:** Context menu > Delete, even for the active tab. If other tabs reference it, a "Delete tabs" warning dialog lists possible conflicts (Delete / Cancel); otherwise deleted without confirmation. Deletion is a history entry; recover via Undo or Restore (DOC-36).
- **Edge cases & errors:** The last remaining tab cannot be deleted. Referencing features/instances fail after deletion (missing reference errors).
- **OCCT mapping:** N/A. App: reverse-reference index to detect dependents before delete.
- **Priority:** P0

### DOC-23 Reorder tabs
- **Purpose:** Control tab order.
- **Sources:** H:Document/document_tabs, H:Document/documents.
- **Selection inputs:** drag tab in tab bar or tab manager.
- **Parameters:** none.
- **Behavior / semantics:** Drag-and-drop; order shared and persistent (DOC-16). Merge/Restore dialogs order tabs by tab-bar position.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App: ordered list of element ids in workspace state (use fractional indexing to make concurrent reorders mergeable).
- **Priority:** P0

### DOC-24 Tab folders
- **Purpose:** Hierarchical grouping of tabs.
- **Sources:** H:Document/document_tabs.
- **Selection inputs:** tabs/folders (drag onto folder; Ctrl+click multi-select in tab manager).
- **Parameters:** folder name.
- **Behavior / semantics:**
  - "+" > Create folder: new folder tab with name field active; drag tabs onto it.
  - When a folder is active the tab bar shows only its tabs; an "All tabs" icon returns to the root; overflow tab lists parent folders when deeper than 1 level.
  - Nest by dropping folder onto folder, max 10 levels deep from root.
  - Folder context menu: Rename, Move to parent folder, Unpack folder (move contents to parent and delete folder), Move to document.
  - Tab manager (filters off): Rename, Add selected to a folder (creates folder immediately), New folder, Unpack folder.
- **Edge cases & errors:** Folders not displayed in Merge/Restore dialogs.
- **OCCT mapping:** N/A. App: tree of folder nodes in workspace state.
- **Priority:** P1

### DOC-25 Tab manager panel
- **Purpose:** Searchable, filterable list of all tabs.
- **Sources:** H:Document/document_tabs. Shortcut Alt/Option+T.
- **Selection inputs:** click tab to open; Ctrl+click multi-select.
- **Parameters:** search text (type-ahead); sort: tab order, A-Z / Z-A, by type (remembered); type filters (Part Studio, Assembly, Drawing, File, Released; only types present shown; Clear); List view / Detail view.
- **Behavior / semantics:** Opens on the left, pushing the Feature/Instances list right. Bottom shows thumbnail of selected tab. Drag to reorder. Multi-select context menu: Move to document, Add selected to folder, Delete (multiple). Folders visible and actionable only when filters are off.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App UI.
- **Priority:** P1

### DOC-26 Tab context menu (workspace vs version)
- **Purpose:** Per-tab commands.
- **Sources:** H:Document/document_tabs, H:PartStudio/part_studios.
- **Selection inputs:** right-click a tab.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - In a version: Open in new browser tab, Properties, Show code (Part Studios, view-only FeatureScript of the feature list), Export, Create task.
  - In a workspace adds: Rename, Duplicate, Copy to clipboard, Select as document thumbnail, Move to document, Delete, Update linked document (if tab has external references), Create drawing of <tab> (Part Studio/Assembly), Release / Release all configurations (P2), Where used (Feature Studio), Change to version (drawings and other referencing tabs), "Open tab, close it for other user" (non-collaborative tabs, DOC-63).
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App UI.
- **Priority:** P0

### DOC-27 File (blob) tabs
- **Purpose:** Hold arbitrary imported files inside a document; act as sources for Import features, Derived/inserted images, CSV/JSON/text references.
- **Sources:** H:Document/importing_files, H:Document/documents, H:PartStudio/variable (CSV "From table").
- **Selection inputs:** none.
- **Parameters:** file name (tab name), media type, bytes (upload limit 4 GB per file).
- **Behavior / semantics:**
  - Every imported file becomes its own tab named after the file; CAD files additionally generate Part Studio/Assembly tabs.
  - PDFs and images are displayed; files without a viewer show file information.
  - File tab buttons: Update (replace the file contents with a new upload; downstream Import features re-translate) and Download.
  - Blob tabs can be referenced from other tabs: Import feature (CAD), sketch Insert image, CSV-driven variables, Variable Studio "From import", custom-feature table/JSON/text parameters; from other documents only via versions.
- **Edge cases & errors:** Failed translations listed in Notifications import queue with retry link.
- **OCCT mapping:** N/A for storage; translation uses STEPControl_Reader / IGESControl_Reader / RWStl / RWObj / RWGltf / XCAF (section 7).
- **Priority:** P1

### DOC-28 Select as document thumbnail
- **Purpose:** Choose which tab's image represents the document.
- **Sources:** H:Document/documents_page, H:Document/document_tabs.
- **Selection inputs:** tab context menu.
- **Parameters:** none.
- **Behavior / semantics:** Thumbnail defaults to last opened Part Studio; explicit choice persists until that tab is deleted/moved (then reverts to most recently edited tab). Requires edit permission. Publications have the equivalent "Select as publication thumbnail".
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App: offscreen render of the element to PNG.
- **Priority:** P2

### 1.3 Workspaces, history, versions, branches, undo

### DOC-29 Workspace
- **Purpose:** The editable state of a document on one branch; where all modeling happens.
- **Sources:** H:Document/document_management, H:Document/versions_and_history, H:Home/glossary ("Workspace").
- **Selection inputs:** none.
- **Parameters:** name (default "Main" for the first; user-named for branches), description, category, workspace units (DOC-54), protection flag (DOC-40), FeatureScript/library version (DOC-41).
- **Behavior / semantics:**
  - A document can have many workspaces, at most one per branch, always at the tip of the branch. The workspace name shown next to the document name identifies it.
  - All tabs of the document exist per workspace; tabs created in one workspace do not appear in others until merged.
  - Multiple users can edit the same workspace simultaneously; changes appear in real time.
  - Within one document, references between tabs normally target the workspace (live updates, e.g. an Assembly instancing a part of a Part Studio in the same workspace), but may also target a version of the same document (DOC-43).
  - Comments are associated with a document and one of its workspaces.
- **Edge cases & errors:** Workspaces created from an old version may be out of date relative to the current FeatureScript release ("Workspace status" notification; Update workspace).
- **OCCT mapping:** N/A. App: mutable pointer (branch head) to the latest microversion of that branch.
- **Priority:** P0

### DOC-30 Autosave and persistent change history
- **Purpose:** No save button: every completed edit by any user is persisted immediately as a history entry; any earlier state can be viewed and restored.
- **Sources:** H:Document/document_management, H:Document/versions_and_history, H:Document/documents.
- **Selection inputs:** none.
- **Parameters:** history entry fields: tab, action, feature/object name, user, timestamp.
- **Behavior / semantics:**
  - Onshape "captures the state of every tab in a workspace every time an edit is completed (by all users working in that workspace)". Each persisted change is a history entry (change node) on the branch.
  - Entry description format: `Tab-name :: Action : Feature-name` (example "Part Studio 1 :: Insert feature : Fillet 1"). Hover shows who and when.
  - History is listed per workspace/version under "Show changes" (collapsed), 25 entries at a time with "Show more"; it is a complete audit trail since document creation regardless of user or tab.
  - Edits in progress inside an open feature dialog are not history entries; the history entry is created when the feature is accepted (inferred from "every time an edit is completed" and dialog undo behavior, DOC-31).
  - Paused regeneration mode (Part Studio): changes applied when resumed; switching tabs/closing applies them (H:PartStudio/features_and_parts_lists).
  - Comments are not recorded in workspace history.
  - Version and merge operations, restores and automatic versions also appear in the graph.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App: append-only microversion log per branch; each commit stores changed element states content-addressed (DOC-66). Commit on feature accept, sketch close, tab operation, property apply, etc.
- **Priority:** P0

### DOC-31 Undo / Redo
- **Purpose:** Session-local reversal of the user's own recent actions.
- **Sources:** H:Home/user_interface_basics, H:Home/keyboard_shortcuts_and_hotkeys, H:Home/dialogs, H:PartStudio/feature_tools, H:Document/merging, H:Drawing/sheets, H:PartStudio/features_and_parts_lists, H:PartStudio/part_studios. Shortcuts: Undo Ctrl/Cmd+Z; Redo Ctrl+Y / Shift+Cmd+Z. Undo/Redo buttons at bottom-left of Feature list (and in sketch toolbar/dialogs).
- **Selection inputs:** none.
- **Parameters:** none.
- **Behavior / semantics:**
  - Scope: "available per user, per tab, per session". Undo reverts the last successful action; Redo re-applies the last undone action; both repeatable.
  - Each user's undo stack contains only that user's actions in that tab during the current session (reload/close clears it) (inferred from "per session").
  - While a sketch or feature dialog is open, Undo/Redo act on individual edits within that editing session. Once the dialog is accepted or cancelled, those in-dialog actions are removed from the Undo/Redo list and the whole editing session becomes one undoable step ("Use the Undo/Redo buttons after closing a sketch or edit dialog to revert an editing session").
  - Undo itself is a new persisted change in history (history is never rewritten) (inferred; consistent with restore model).
  - Things that cannot be undone with Undo: a Merge (use the "revert merge" link offered in the post-merge message), changes regenerated from Paused regeneration mode after the green check (use restore), workspace deletion via Delete permission (warning: cannot be undone; restore via Restore deleted workspaces).
  - Drawing sheet deletion is undoable. Properties dialog has its own Undo/Redo.
  - Difference from Restore: Undo is short-term, per-user, per-tab; Restore (DOC-36) can return any tab or the whole workspace to any point.
- **Edge cases & errors:** In collaborative editing, undoing an action that another user built upon may fail or produce rebuild errors (inferred; not documented). Undo is unavailable in versions (read-only).
- **OCCT mapping:** N/A. App: per (user, element) stack of operation records referencing (before microversion, after microversion); undo = commit that restores the element state from "before" if no intervening foreign changes to the same element, else apply inverse operation (DOC-69).
- **Priority:** P0

### DOC-32 Versions
- **Purpose:** Named, immutable snapshot of the entire document (all tabs) at a point in time; required for cross-document references.
- **Sources:** H:Document/versions_and_history, H:Document/document_management, H:Home/glossary ("Version").
- **Selection inputs:** a workspace (or a history entry: "Create version here").
- **Parameters (Create version dialog):**

| Field | UI label | Default / notes |
|---|---|---|
| name | Name | `V<n>` incrementing from V1 (first created is V1; "Start" exists from creation) |
| description | Description | optional |
| (action) | Create / Create version and edit properties | latter opens Version properties dialog for tab/part names and descriptions |

- **Behavior / semantics:**
  - Create version icon in Document panel creates a version of the current workspace without leaving it; also from Versions and history panel and from any change entry ("Create version here").
  - Versions are view-only; geometry, part names etc. immutable; version name/description/properties remain editable.
  - If drawings have pending updates (yellow Update icon), the dialog lists them with an update action so the version contains up-to-date drawings (clicking update then cancelling still updates drawings).
  - Automatic versions: created by Onshape when "Update all" references needs new versions in linked documents (special icon; can be hidden in the graph).
  - Versions can be opened (read-only), branched, compared, merged from, copied (Copy version), linked to from other documents.
  - Versions cannot be deleted.
  - Release candidates/releases are versions with release state (P2, out of scope).
  - Published custom features show `{}` around the version node.
- **Edge cases & errors:** Inserting from another document requires a version; dialogs offer inline "Create version" (DOC-44). Operation that would modify a saved version errors (ErrorStringEnum RESTRUCTURE_CANNOT_MODIFY_SAVED_VERSION "Operation cannot be completed as it would modify a saved version.").
- **OCCT mapping:** N/A. App: version = named immutable pointer to a microversion id + mutable metadata record.
- **Priority:** P1

### DOC-33 Branches, creating/deleting/restoring workspaces
- **Purpose:** Parallel lines of development forked from a version.
- **Sources:** H:Document/versions_and_history, H:Document/document_management, H:Collaboration/share_documents (Delete permission).
- **Selection inputs:** a version (context menu > Branch to create workspace); a branch (Delete).
- **Parameters (Branch dialog):** name, description, "open" checkbox (open new workspace after create); buttons Create / Create and edit properties.
- **Behavior / semantics:**
  - A branch forks at a version, ends with up to one workspace, and holds 0..N sequential versions. Branch names bold italic in graph; open dot = has workspace, closed dot = workspace deleted.
  - New workspace starts identical to the source version; inherits that version's workspace units.
  - Delete workspace (owner or users with Delete permission): if branch has no versions, workspace and branch are deleted; otherwise only the workspace is deleted and the branch tip reverts to its latest version. Main workspace and versions cannot be deleted. Confirmation dialog.
  - Restore deleted workspaces: Document menu > Restore deleted workspaces > select branches > Restore selected.
  - From a branch with deleted workspace, a new workspace can be created from any version on it.
- **Edge cases & errors:** Share help: "when deleting a workspace, a warning dialog displays and the action, if taken, cannot be undone" (it is not on the Undo stack; Versions help nevertheless documents Restore deleted workspaces).
- **OCCT mapping:** N/A. App: branch record {id, name, fork version, head microversion | null, deleted flag}.
- **Priority:** P1

### DOC-34 Versions and history panel (version graph)
- **Purpose:** Visualize and act on the document's branch/version/history DAG.
- **Sources:** H:Document/versions_and_history, H:Document/document_management, H:Document/merging.
- **Selection inputs:** click node to view; right-click for context menu.
- **Parameters / toolbar:** Create release (P2), Create version, Compare history entries, Repair (DOC-42), Active branch toggle (All branches / Active branch), Filter branches (Show all branches; Show all branches with workspaces; Show current branch; Show current branch with parent branches), Show/Hide merges, Show/Hide automatic versions, Search history.
- **Behavior / semantics:**
  - Graph is a tree colored per branch; workspaces = open dots at branch tips; versions = solid dots (normal text); change nodes = history entries; releases = triangles (solid released, open pending, solid with dot obsoleted); automatic versions have their own icon; merge lines connect source to target for every merge; active workspace highlighted dark blue with a blue box.
  - Hover an entry: user and time; version description shown on hover.
  - Click a history entry/version: visualize the document at that point (read-only); "Return to <branch-name>" goes back to the live workspace.
  - Context menu (varies with node type): Open, Open in new browser tab, Copy link (URL to that change/version/branch), Compare, Merge into current workspace, View in repair, Restore to <current workspace>, Create version here, Branch to create workspace (versions), Workspace protections, Properties, Publish FeatureScript, Delete (branch), Scroll to branch top / bottom.
  - Search history: term matches history entries across all branches (ignores Active branch filter; version names/descriptions not searched); dropdown filters Tab (Any tab), Tab types (Part Studio/Assembly/Drawing), Modified by (Only me / Not me / Any), Date modified (start/end); Search, Reset.
  - Also available from Documents page right panel.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App UI: DAG layout (lanes per branch) over microversion graph; history entries = microversions with commit messages.
- **Priority:** P1

### DOC-35 Viewing a past state / opening a version
- **Purpose:** Read-only inspection of any history entry or version.
- **Sources:** H:Document/versions_and_history, H:Document/document_management, H:Document/document_tabs.
- **Selection inputs:** history entry or version.
- **Parameters:** none.
- **Behavior / semantics:** Opening shows the full document (all tabs) at that state, view-only; toolbar shows version name instead of workspace; tab context menu reduced (DOC-26); Document menu shows Copy version / Version properties. URL identifies the version or change so it can be shared (Copy link). Version-state Part Studios regenerate deterministically from stored feature lists (inferred).
- **Edge cases & errors:** Comments panel unavailable in versions (no edit rights).
- **OCCT mapping:** N/A. App: open element states at a microversion id; geometry from cache or regenerate.
- **Priority:** P1

### DOC-36 Restore (to a point in history)
- **Purpose:** Return the current workspace (all or selected tabs) to the state of an earlier history entry or version of the same branch.
- **Sources:** H:Document/restoring, H:Document/versions_and_history.
- **Selection inputs:** right-click history entry/version > "Restore to <workspace>".
- **Parameters (Restore dialog):**

| Field | Values | Notes |
|---|---|---|
| Overall restore strategy | Keep [Target] / Replace with [Source] | changing it switches all changed tabs to that strategy |
| Tab restore strategy (per tab) | Keep [Target] / Replace with [Source] | overrides overall |
| "N changes" link | - | opens Compare (source vs target) in new browser tab |

- **Behavior / semantics:**
  - The workspace that is currently open is the one restored; to restore another workspace open it first.
  - Per-tab result icons: green check = tab's changes persist; red X = tab replaced by older state; exclamation = tab will not exist after restore (e.g. created after the restore point).
  - Tabs ordered: changed first, then unchanged; within each by tab-bar position, target first then source; folders not shown.
  - To restore only some tabs: Overall = Keep, set chosen tabs to Replace.
  - The restore is recorded as the newest change on the branch (history is not truncated), so a restore can itself be undone by restoring to the entry before it.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** N/A. App: new microversion whose element map = target's map with selected element states replaced by source's states (element add/remove included).
- **Priority:** P1

### DOC-37 Compare
- **Purpose:** Graphical and list diff between any two history points (workspaces, versions, change entries).
- **Sources:** H:Document/compare, H:Document/versions_and_history. Available in Part Studio and Feature Studio.
- **Selection inputs:** Compare icon then pick two entries (first = blue/Base, second = red/Target); or context menu > Compare on a target (current = Base). Expand "Changes" to pick individual entries.
- **Parameters:** Reverse compare; Show differences only vs list all features; Part Studio selector tab (lists all Part Studios in document); Configuration selector when configured; Base/Target visibility slider.
- **Behavior / semantics:**
  - Side-by-side feature lists (Base left, Target right) with icons: none = identical; not-equal = differs; less-than = only in Base; greater-than = only in Target; up/down arrows = same feature at a different list position.
  - Graphics overlay: Base geometry blue, Target red; slider blends visibility toward either side; part context menu Show/Hide part to isolate changes.
  - Selecting a feature compares its parameters; differing tolerance options are outlined yellow.
  - Feature Studios: line-by-line text diff, additions green, deletions red.
- **Edge cases & errors:** Assemblies/drawings not listed as supported for Compare.
- **OCCT mapping:** Geometry diff display: regenerate both states; optionally BRepAlgoAPI_Cut both ways for added/removed volume highlighting (custom). Feature list diff: match features by persistent feature id, compare serialized parameters.
- **Priority:** P2

### DOC-38 Merge
- **Purpose:** Apply changes from a source (workspace or version) into the current workspace (target).
- **Sources:** H:Document/merging, H:Document/versions_and_history.
- **Selection inputs:** select target workspace (active, dark blue), right-click source > "Merge into current workspace".
- **Parameters (Merge dialog):**

| Field | Values | Default / notes |
|---|---|---|
| Overall merge strategy | Keep [Target] / Merge changes / Replace with [Source] | Merge changes is default; if no changes need merging only Keep/Replace offered with Replace default |
| Tab merge strategy (per tab) | same three, or only Keep/Replace for created/deleted tabs and non-mergeable types | overrides shown highlighted yellow |
| Compare with base | link | opens Compare between source and base in new browser tab |

- **Behavior / semantics:**
  - Three-way: delta from base to source is applied onto target. Base = common ancestor (fork point), or the last point where the same source branch was previously merged into this target (if that merge was not reverted).
  - Granularity is per tab: cannot pick individual changes within a tab.
  - "Merge changes" only for Part Studios and Assemblies (help video) / all types except Drawings and PCB Studios (help text), and only when both sides changed that tab. Part Studio changes merged: create/modify/delete features, configuration inputs, materials, appearances. Assembly changes merged: add/modify/remove components (instances), mate connectors, configurations. Incompatible combined changes may cause feature errors.
  - Keep = target tab unchanged; Replace = target tab overwritten by source tab.
  - Icons per tab: green check (kept edits), red X (discarded edits, e.g. deleted features, are carried into the target), exclamation (tab will not exist after merge), info (type cannot be merged; choose Keep or Replace).
  - Tab ordering as in Restore dialog; folders not shown.
  - Merge recorded in the graph as a merge line; can also merge a version from another branch.
  - After merge a persistent message offers "revert merge" (merge is not undoable via Undo); reverting discards any changes made after the merge; closing the message accepts.
  - Partial merge recipe: Overall = Keep, specific tabs = Replace or Merge.
- **Edge cases & errors:** see DOC-39.
- **OCCT mapping:** N/A. App: three-way merge of element states keyed by element id; within Part Studio, three-way merge of feature list keyed by feature id (DOC-70).
- **Priority:** P2

### DOC-39 Merge blockers and conflict rules
- **Purpose:** Conditions preventing or degrading merges.
- **Sources:** H:Document/merging, H:Document/workspace_protections.
- **Selection inputs:** none.
- **Parameters:** none.
- **Behavior / semantics:**
  - Drawings (and PCB Studios) edited on both branches: only Keep or Replace; one side's edits are lost. Recommended: edit a drawing on one branch only.
  - No changes: if source/target identical relative to base and all tabs are Keep, Merge button disabled ("No changes" means delta base->source).
  - Protected target workspace out of date with working branch: Merge disabled with "Workspace protections are enabled" warning; fix by merging protected into working branch first, then working into protected.
  - FeatureScript version mismatch: branches on outdated FS versions cannot be merged; dialog offers Update FeatureScript (updates both target and source), then Merge.
  - Document open elsewhere: merge blocked if other users or the same user in other windows have the document open; dialog lists users and instance counts, auto-refreshes; network uncertainty shows Retry.
  - Update scheduled: merges blocked while workspace updates in progress.
- **Edge cases & errors:** as above.
- **OCCT mapping:** N/A. App: for a local app, "document open elsewhere" maps to an exclusive lock during merge; FS version mismatch maps to "feature-schema version mismatch" requiring migration (DOC-41).
- **Priority:** P2

### DOC-40 Workspace protections
- **Purpose:** Prevent direct edits on an important branch; force branch-and-merge workflow.
- **Sources:** H:Document/workspace_protections.
- **Selection inputs:** right-click workspace > Workspace protections.
- **Parameters:** Enable workspace protection (checkbox), Apply.
- **Behavior / semantics:**
  - Must be done on a branch and merged: geometry changes, drawing changes, certain metadata changes, creating/deleting tabs.
  - Allowed on protected workspace: release management and required metadata, refresh drawings, version creation, branching, merging.
  - Lock icon in graph; banner when the protected workspace is active; editing tools (add tab, sketch, edit geometry) disabled.
  - Not read-only: anyone with edit permission can branch, edit and merge in.
  - Any user with Edit can toggle (non-enterprise); enterprise admins restrict via Global permissions. Mobile honors but cannot toggle.
- **Edge cases & errors:** Merge into protected requires working branch to be up to date (DOC-39).
- **OCCT mapping:** N/A. App: branch flag checked by command layer.
- **Priority:** P2

### DOC-41 Update workspace (FeatureScript library version)
- **Purpose:** Upgrade a workspace created under an older Onshape/FeatureScript standard library version to the current one.
- **Sources:** H:Document/versions_and_history ("Updating a workspace"), H:Document/merging (FS version mismatch), H:Document/documents_page (Workspace status notification); FS: `FeatureScriptVersionNumber` enum (featurescriptversionnumber.gen.fs, ~1968 entries, current `V2960_DELETE_FAILED_FLAT_BY_PART`), `isAtVersionOrLater(context, version)`.
- **Selection inputs:** none.
- **Parameters:** Document menu > Update workspace > Update now.
- **Behavior / semantics:**
  - Every Part Studio regenerates with the standard library version pinned in the workspace; feature code branches on `isAtVersionOrLater` so old models regenerate identically. Updating moves the pin to the latest version (geometry may change slightly where behavior was fixed).
  - Document is inaccessible during the update; Documents page shows an update icon/tooltip; repeat per workspace.
  - Workspaces branched from or restored to an old version become out of date (notification).
  - Feature serialization also carries versions (ErrorStringEnum FEATURE_BAD_SERIALIZATION_VERSION "Bad serialization version", FEATURE_WRONG_SERIALIZATION_VERSION "Wrong serialization version").
- **Edge cases & errors:** Merge blocked on mismatch (DOC-39).
- **OCCT mapping:** N/A. App: store `kernel_behavior_version` per workspace/element; feature implementations must gate behavior changes on it (critical for deterministic regeneration with evolving OCCT versions).
- **Priority:** P1

### DOC-42 Repair: View in repair, Edit healthy moment, Replace reference
- **Purpose:** History-assisted fixing of broken references after upstream changes (typically after re-pointing a Derived or Import feature).
- **Sources:** H:Document/repairing.
- **Selection inputs:** a history change entry (Versions panel Repair icon or context menu "View in repair"); a failed feature ("Edit healthy moment of <Feature>"); a broken selection inside a feature dialog ("Edit healthy moment", "Replace reference" icon).
- **Parameters (Replace reference dialog):** replacement selection (Feature list or graphics); Propagate changes (checkbox, default on) with read-only list "Propagating changes to features".
- **Behavior / semantics:**
  - Repair panel opens side-by-side: live Part Studio left, chosen healthy state (read-only) right; Synchronize view toggle (default on); hovering a broken reference highlights where it pointed in the healthy state; Repair panel context menu Zoom to fit, Isometric.
  - Edit healthy moment = the feature's last successful regeneration state.
  - Replace reference with Propagate: substitutes the new entity for the broken one in this feature and all downstream features that used the same reference. Fix top-down.
- **Edge cases & errors:** Edit healthy moment unavailable when references are missing but the feature does not fail (use Versions panel instead).
- **OCCT mapping:** Needs persistent naming history (TNaming / custom topological naming) to map old references; UI pairs two regenerated contexts. Custom work.
- **Priority:** P1

### 1.4 Linked documents and references

### DOC-43 Linked documents (version-based reference model)
- **Purpose:** Reuse data from another document (or another version of the same document) through immutable version references with explicit, user-controlled updates.
- **Sources:** H:Document/linking_documents, H:Assembly/updating_references, H:PartStudio/derived.
- **Selection inputs:** none (created by Insert, Derived, Move to document, Variable Studio insert, custom features, drawings changed to version, publications).
- **Parameters:** reference = (document id, version id, element id, [configuration], [part id(s)/query]); same-document references may instead target the workspace.
- **Behavior / semantics:**
  - Within one document and workspace, references are live: changes to parts propagate instantly to assemblies/drawings in the same workspace.
  - Cross-document references always point to a version (immutable). Hence every state of the using document (and its versions) is stable and recoverable.
  - When the referenced document gets a newer version on the same branch with edits in the referenced tab, the using document shows a notification (blue background on link icon in Feature list/Instances list and on the tab); the user decides whether/when to update (DOC-46). Versions without edits to the referenced tab do not trigger the blue icon.
  - References to different versions of the same document are allowed (e.g. derive from V3 of own document).
  - Circular references are rejected (Derived: A->B->A fails).
  - Unsharing does not break existing links (DOC-03).
  - Linked documents can be exported (browser).
  - Self-reference by element id forbidden for assemblies inserted as version references (DOC-15).
- **Edge cases & errors:** Linked document trashed / deleted / not permitted errors (DOC-48). ErrorStringEnum REST_ASSEMBLY_EXTERNAL_REFERENCE_REQUIRES_VERSION "Linked document references require a version identifier".
- **OCCT mapping:** N/A. App: reference record stored in the referencing feature/instance; resolution loads the referenced element at that version from the other store (read-only) and regenerates/caches its output (shape cache keyed by (doc, microversion, element, configuration)).
- **Priority:** P1

### DOC-44 Insert from other documents (version selection and inline versioning)
- **Purpose:** Common flow for picking an element/part from the current document or another document.
- **Sources:** H:Assembly/insert_parts_and_assemblies, H:Document/linking_documents, H:PartStudio/derived, H:VariableStudio/variable_studio, H:PartStudio/add_custom_features. Assembly Insert shortcut: i.
- **Selection inputs:** document (via filters, teams, labels, folders, search, pasted URL), then element/part/sketch/surface/curve/mate connector/variable.
- **Parameters:**

| Control | Values / notes |
|---|---|
| Source | Current document / Other documents / Standard content (assemblies) |
| Version | latest version selected by default; Version graph icon to pick an older version; Create version link if none |
| Released items filter | pick a specific revision (P2) |
| Type filters | Part Studios, Assemblies, parts, sketches, surfaces, curves, mate connectors, meshes |
| Configuration inputs | set before inserting configured items ("Generate"; "Set to last viewed configuration", "Reset to default") |

- **Behavior / semantics:**
  - Current document: default reference target is the workspace (live) for Insert; Derived lets you choose workspace or a version of the current document.
  - Other documents: only versions can be referenced; if the document has no version (or a newer one exists) a notice is shown with a link to create a version immediately (requires edit permission on that document).
  - Last chosen source (current/other/standard) remembered.
  - After insertion: "Change to version..." on an instance changes its version; Open linked document opens the source at that version.
- **Edge cases & errors:** Cannot insert from a document without versions if user lacks edit permission there. Duplicate insertion of the same tab into a Publication is blocked.
- **OCCT mapping:** N/A. App UI + reference creation.
- **Priority:** P1

### DOC-45 Derived feature: linking and update model
- **Purpose:** Bring parts/sketches/surfaces/curves/planes/mate connectors/whole Part Studios from another Part Studio (same doc or another doc) into the current Part Studio with a one-way associative link. Full parameter spec is in section 3 (FS `importDerived`, importDerived.fs); this item covers the document-model aspects only.
- **Sources:** H:PartStudio/derived; FS: `importDerived` (importDerived.fs), `PartStudioData` (tabReferences.fs).
- **Selection inputs:** Part Studio (one per Derived feature) and entities within it (see section 3).
- **Parameters:** see section 3. Reference-relevant fields: `partStudio` (PartStudioData: buildFunction, configuration, partQuery, configurationData), `includeProperties` (copy metadata vs only name/material/appearance), `preserveActiveSheetMetal`.
- **Behavior / semantics:**
  - Same document: may reference the workspace (updates instantly; slower) or a version (recommended for performance; update manually). Other document: always a version.
  - One-way: changes in the base propagate to the derived copy (on regeneration for workspace refs; on reference update for version refs), never reverse.
  - Link icon on the Derived feature in the Feature list; turns blue and an identical icon appears on the Part Studio tab when a newer version is available; clicking opens the Reference manager.
  - Swapping the source (edit feature, pick another Part Studio/document) is allowed; downstream failures are repaired with DOC-42.
  - Child features (sketches, planes, mate connectors, patterns, mirrors, transforms) listed under the Derived feature.
  - Best practice: derive from the original, never chain Derived-of-Derived; derive mature parts or simple Part Studios/sketches.
  - Build function semantics: the referenced Part Studio is executed as a function of its configuration in a separate context (`definePartStudio` returns `function(configuration) returns Context`) and selected bodies are instantiated into the current context (Instantiator).
- **Edge cases & errors:** ErrorStringEnum: DERIVED_NO_PARTS "Select entities from another Part Studio."; DERIVED_NO_SAME_SOURCE "Creating multiple Derived features from the same source and configuration is not allowed. Consider editing the previous Derived feature."; DERIVED_NO_INSTANCING "Using derived feature to make instances is not recommended. Consider using an Assembly instead."; IMPORT_DERIVED_NO_PARTS "Failed to recreate derived objects."; DERIVED_BODIES_HAVE_FAULTS "Parts were derived but contained faults."; DERIVED_MATE_CONNECTOR_NO_LONGER_VALID; DERIVED_MATE_CONNECTOR_RESET; DERIVED_NO_MATE_CONNECTORS. Circular references fail. Visibility of the source part is ignored (derived parts always visible).
- **OCCT mapping:** Regenerate source element (cached by (doc, microversion, element, config)); copy TopoDS_Shape with BRepBuilderAPI_Copy / location transform (BRepBuilderAPI_Transform or TopLoc_Location) to placement mate connector; persistent ids must be namespaced by derived feature id.
- **Priority:** P1

### DOC-46 Update notifications and Reference manager
- **Purpose:** Show out-of-date references and update them (all, selective, pinned, recursive with automatic versions).
- **Sources:** H:Assembly/updating_references, H:Document/linking_documents, H:PartStudio/add_custom_features, H:PartStudio/variable_table.
- **Selection inputs:** click a link/revision icon next to an instance/feature (opens Reference manager), Update all button (Assembly toolbar), context menu "Update linked document", "Pin reference".
- **Parameters (Reference manager):**

| Control | Values / notes |
|---|---|
| Mode | Update to latest / Selective update |
| Per-reference checkbox + version picker | Version graph icon to choose any version (incl. older) |
| Filters | Pinned only; Newer revision available |
| Branch choice | when newer versions exist on several branches, prompt to choose workspace/branch |
| Update to the new document | for instances whose source was moved to another document |
| Buttons | Update all / Update selected |

- **Behavior / semantics:**
  - Icon states (version icon; triangle = revision; thumbtack = pinned): up-to-date; blue background = newer version/revision available; pinned variants (open grey circle when newer available); solid grey circle = child of a top-level instance out of date (navigate into subassembly); small arrow with blue/grey background on subassemblies containing out-of-date children. Out-of-date state propagates up the hierarchy.
  - Same-document workspace subassemblies show child out-of-date indicators too.
  - Update all is recursive: for each out-of-date leaf it updates to the newer version; where an intermediate linked document has only workspace changes ("new version" needed), Onshape automatically creates versions in those documents bottom-up and updates parents to them (automatic versions appear in their graphs). All references updated to latest even those not out of date.
  - Pinned references are excluded from Update all; update them via Selective update.
  - Revision updates must be done at the subassembly level first, then pulled up.
  - Custom features: blue icon is notification only; update via Reference manager; tools added by admins may show "out-of-sync".
  - Variable Studio references: "Update linked document" opens Reference manager.
- **Edge cases & errors:** Update disabled if any listed document lacks write permission (needed to create automatic versions).
- **OCCT mapping:** N/A. App: reference index per element; "latest version on branch with changes to element" query = walk referenced doc's version list and compare element content hashes (content addressing makes "has edits in this tab" a hash comparison).
- **Priority:** P1

### DOC-47 Change to version / Update linked document on tabs
- **Purpose:** Switch a tab-level or instance-level reference between workspace and a specific version.
- **Sources:** H:Document/document_tabs (drawing "Change to version"), H:Assembly/insert_parts_and_assemblies ("Change to version..."), H:PartStudio/part_studios ("Update linked document").
- **Selection inputs:** tab or instance.
- **Parameters:** target version.
- **Behavior / semantics:** A drawing changed to a version stops reflecting model changes (no out-of-date indication for workspace edits) until updated to a newer version; enables moving the model or the drawing to separate documents. Duplicated + moved drawings stay linked to the original document version.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A.
- **Priority:** P1

### DOC-48 Open linked document and link errors
- **Purpose:** Navigate to the source of a linked item.
- **Sources:** H:Document/linking_documents, H:PartStudio/variable_table.
- **Selection inputs:** linked instance/feature/variable table (context menu > Open linked document).
- **Parameters:** none.
- **Behavior / semantics:** Opens a new browser tab with the source document at the linked version and the source Part Studio active. Editing the source in its workspace then versioning makes the blue update icon appear in the using document.
- **Edge cases & errors (messages):**
  - Source in trash: "Failed to load document for workspace. Cannot open a document in the trash. Restore the document from Trash."
  - Source permanently deleted: "Failed to load document for version. Resource does not exist, or you do not have permission to access it."
  - No access to source: "You cannot modify this feature because you cannot access the referenced document."
- **OCCT mapping:** N/A.
- **Priority:** P1

### DOC-49 Linked custom features and Feature Studio imports
- **Purpose:** Custom features (FeatureScript) are added to toolbars and Part Studios by version reference to a document containing Feature Studios.
- **Sources:** H:PartStudio/add_custom_features, H:FeatureStudio/feature_studios, H:Document/where_used.
- **Selection inputs:** document containing FeatureScript (search or URL).
- **Parameters:** document, version (latest default), feature(s).
- **Behavior / semantics:** Features from the current workspace update automatically when the Feature Studio is edited; features from other documents come from a version and update via Reference manager; Feature Studios import other Feature Studios and the standard library by version (`import(path : ..., version : ...)`); Where used on a Feature Studio tab lists document versions using it. Requires at least "View, copy & export".
- **Edge cases & errors:** Feature list containing a custom feature whose icon is not on the user's toolbar shows it under "Linked custom features" (inferred from truncated text).
- **OCCT mapping:** N/A. noshape has no FeatureScript runtime initially; equivalent would be a plugin/scripting system with versioned modules.
- **Priority:** P2

### DOC-50 Where used
- **Purpose:** Find which assemblies and drawings (versions/revisions) use a part or assembly, or which document versions use a Feature Studio.
- **Sources:** H:Document/where_used.
- **Selection inputs:** part/assembly (Parts list, Instances list, graphics context menu, Documents page search result), Feature Studio tab.
- **Parameters:** Part version selector (version/revision of the queried item; "Show more" lists each part of multi-part Part Studios per version); results filter Latest revision / All revisions / Latest versions / All versions.
- **Behavior / semantics:** Results tree from newest to oldest; each row: item at version used in parent at version with release state and part number; recursive up to top-level assemblies; configured parts expand to used configurations; right-click result for nested Where used or open in Isolate mode. Workspaces are never included. "Not permitted" and "Failed captures" markers.
- **Edge cases & errors:** Started from a workspace returns no results until a version is selected.
- **OCCT mapping:** N/A. App: global reverse index of version references across the local library.
- **Priority:** P2

### DOC-51 Publications (brief)
- **Purpose:** View-only curated collection of versioned/revisioned tabs from many documents for sharing.
- **Sources:** H:Document/publications.
- **Selection inputs:** tabs/released parts from documents with link permission.
- **Parameters:** name, description, labels, Force publication Export rules, notes (Markdown).
- **Behavior / semantics:** Insert document tabs (versions only, latest default, version graph picker, revision picker, Create a version link; duplicates blocked); Publication manager works like Tab manager (folders, reorder, delete, list/detail view); blue update notification for newer versions/revisions; View as viewer; Copy publication; share view-only.
- **Edge cases & errors:** Individual parts insertable only if released.
- **OCCT mapping:** N/A. App: special document kind containing only external references.
- **Priority:** P2

### DOC-52 Structure view (brief)
- **Purpose:** Documents-page tree of the latest released parts/assemblies/drawings per folder, top-level assembly downward.
- **Sources:** H:Document/structure_view.
- **Selection inputs:** row (not the name link).
- **Parameters:** columns (Name fixed; Description, Part number, Revision default; Vendor, Project, Product line, Title 1/2...), Show drawings toggle, column pin/sort/autosize/reset.
- **Behavior / semantics:** Only latest releases; unused released components at top; newer revision highlighted blue; obsolete icon; right panel Details, Properties, Where used, Revision history; row menu Open in new tab, Add to publication, Create task, Export, Details, Collapse all row groups.
- **Edge cases & errors:** Professional/Enterprise only; depends on release management (out of noshape CAD scope).
- **OCCT mapping:** N/A.
- **Priority:** P2

### DOC-53 Variable Studios and Variable tables as cross-tab / cross-document variable sources
- **Purpose:** Define variables once (Variable Studio tab) and consume them in Part Studios/Assemblies of the same workspace or, by version reference, other documents. (Variable feature parameters are in the Part Studio section; configured Variable Studios in the configurations section.)
- **Sources:** H:VariableStudio/variable_studio, H:PartStudio/variable_table, H:PartStudio/variable.
- **Selection inputs:** Insert Variable Studio dialog: Current document / Other documents; whole Variable Studio table or individual variables.
- **Parameters (Variable Studio row):**

| Column | Type | Notes |
|---|---|---|
| Name | identifier | Variable Studio says alphanumeric only; Variable table requires FeatureScript identifier `[A-Za-z_][A-Za-z0-9_]*` |
| Type | Length (default) / Angle / Number / Any / From import | From import = CSV-driven (row/column access, result type Single value/Array/Map, conversion factor) |
| Value | expression | may reference variables defined earlier in the same table |
| Description | string | |

- **Behavior / semantics:**
  - Checkbox at bottom of the Variable Studio makes its variables available to all Part Studios and Assemblies in the workspace (leave unchecked while drafting).
  - Rows reorder via drag or Move up/down; collapse/expand tables.
  - Inserting from another document requires a version containing the variables (prompt to create); inserted tables are read-only and titled with source name and version (e.g. "Variable Studio 1 V1"); configured Variable Studios need configuration chosen at insertion.
  - Part Studio/Assembly Variable table panel (right side): create local variables (same columns), Create Variable Studio / Insert Variable Studio, table menu Copy table (spreadsheet round-trip; header rows read-only), Switch to Variable Studio, Open linked document, Update linked document (Reference manager), Move up/down, Edit reference, Remove reference; cell menu Filter feature list, Delete, Move up/down; hovering a variable highlights the defining and consuming features.
  - Name collisions: unique within a table (attempt cancelled with alert); same name in two tables -> later table wins, earlier marked with override icon; a Variable feature in the Feature list with the same name overrides the table variable.
  - Renaming in the table updates all referencing features.
  - Variable Studios can be released (not when configured) (P2).
- **Edge cases & errors:** A Variable feature whose name is not a valid identifier does not appear in the Variable table.
- **OCCT mapping:** N/A. App: variable scope chain (DOC-59).
- **Priority:** P1

### 1.5 Units and numeric expressions

### DOC-54 Workspace default units
- **Purpose:** Per-workspace default units and display precision for all Part Studios and Assemblies in that workspace.
- **Sources:** H:Document/setting_default_units_per_workspace, H:Document/document_menu, H:Document/documents_page (mobile account preferences: Length, Angle, Mass); FS: units.fs (`STRING_TO_UNIT_MAP`), valueBounds.fs, partStudio.fs (`definePartStudio(partStudio, defaultLengthUnit, defaults)`).
- **Selection inputs:** none.
- **Parameters (Document menu > Workspace units):**

| Setting | Values | Default / notes |
|---|---|---|
| Length | Millimeter, Centimeter, Meter, Inch, Foot, Yard (from FS length units; dialog list inferred) | Account preference; Onshape default Inch |
| Length decimal places | display precision (count not documented; inferred 0..~8) | display only |
| Angle | Degree, Radian | Degree |
| Angle decimal places | display precision | display only |
| Mass | Kilogram, Gram, Pound, Ounce (from FS mass units; inferred list) | account preference |
| Mass decimal places | display precision (inferred) | |

- **Behavior / semantics:**
  - Applies to all measurements in Part Studios and Assemblies: sketch dimension display, default input unit for every numeric field in feature dialogs, manipulator dialogs, Measure and Mass properties tools.
  - Initialized from account Preferences > Units at workspace creation; new workspaces branched from a version inherit that version's settings (units are part of versioned workspace state).
  - Typing an explicit unit in a field always overrides; value is converted and displayed in default units.
  - Decimal places: browser only; affect display in feature dialogs, sketch dimensions, manipulators, Measure, Mass properties; not used for computation; values rounded for display only; more entered digits are kept and shown when the field is edited. Measure shows scientific notation if precision insufficient; Mass properties shows measurement error.
  - Do not affect imported files; Drawings have their own units (and only drawings offer Feet and inches).
  - Feature default values depend on the workspace length unit: each bound spec stores a "nice" default per unit, not a conversion. Example NONNEGATIVE_LENGTH_BOUNDS: meter [min 1e-5, default 0.025, max 500], centimeter 2.5, millimeter 25, inch 1, foot 0.1, yard 0.025. Angle specs give degree and radian defaults (e.g. CHAMFER_ANGLE_BOUNDS degree [0.1, 45, 179.9], radian 0.25*PI). The Part Studio function receives the default length unit (`definePartStudio(..., defaultLengthUnit, ...)`) and `addDefaultFeatures(context, defaultLengthUnit)`.
  - Changing workspace units does not change geometry (values are stored in SI internally with their entered expression) (inferred from "values are converted").
- **Edge cases & errors:** none documented.
- **OCCT mapping:** N/A. App: store all quantities in SI (meter, radian, kilogram) as f64 plus original expression string; unit setting only affects parsing default and display formatting. Note OCCT models in mm by convention only; noshape should pick one internal length unit (recommend meter to match FS bounds, or mm with consistent scaling) and keep Precision::Confusion consistent (1e-7 in model units).
- **Priority:** P0

### DOC-55 Numeric field entry and display
- **Purpose:** Behavior of every numeric input (feature dialogs, sketch dimensions, mates, variable values).
- **Sources:** H:Home/numeric_fields, H:Home/dialogs, H:PartStudio/variable.
- **Selection inputs:** none.
- **Parameters:** scroll increments:

| Input | Normal fields | Fields restricted to 0..1 (e.g. isoparametric) |
|---|---|---|
| Scroll wheel | +/-0.1 | +/-1.0 (as documented; likely meant 0.1) |
| Ctrl+scroll | +/-0.01 | +/-0.01 |
| Shift+scroll | +/-1.0 | no effect |

- **Behavior / semantics:**
  - Accepts integers, decimals (comma accepted as decimal separator when Decimal format preference = comma), fractions (e.g. `1/4 in`), parameter expressions, functions, variables.
  - No unit entered -> workspace default unit (for length/angle fields).
  - After accept the field shows the evaluated value in default units; when re-focused, the original text (expression/units as entered) is shown again.
  - Invalid expression: field outlined red and accept disabled (mobile description; desktop equivalent inferred).
  - Typing `#`, `_` or a letter opens variable autofill (DOC-59).
  - Sketch dimensions show an "expression without variable" or "expression with variable" indicator icon left of the value.
  - Fields carry bounds (from FS bound specs); out-of-range values are errors (feature-specific; see section 3).
  - Enter accepts dialog; Shift+Enter accepts and re-opens the same feature empty.
- **Edge cases & errors:** With comma decimal separator, array literals need a space after commas (`[1, 4, 9, 16][#i]`, not `[1,4,9,16][#i]`).
- **OCCT mapping:** N/A. App: GPUI numeric input component with parser (DOC-56), scroll handler, focus/blur display switching.
- **Priority:** P0

### DOC-56 Expression language: grammar
- **Purpose:** Exact syntax accepted in numeric fields. Expressions are evaluated as FeatureScript.
- **Sources:** H:Home/numeric_fields, H:PartStudio/variable; FS: units.fs, math.fs, variable.fs.
- **Selection inputs:** none.
- **Parameters:** token classes:

| Class | Accepted forms |
|---|---|
| Number | integer `5`, decimal `2.5` (or `2,5` with comma format), scientific `1e-3` (FeatureScript literal; inferred) |
| Unit suffix | after a number, parenthesized expression or function call, optional space: `5mm`, `10 meters`, `(3*pi) in`, `sqrt(16)m`; unit powers `in^2` |
| Length units | `mm` `millimeter` `cm` `centimeter` `m` `meter` `in` `inch` `ft` `foot` `yd` `yard` + plurals (`millimeters`, `meters`, `inches`, `feet`, `yards`, ...) |
| Angle units | `deg` `degree` `rad` `radian` + plurals |
| Mass units | `kg` `kilogram` `g` `gram` `oz` `ounce` `lb` `pound` (in FS STRING_TO_UNIT_MAP; not listed in help; inferred usable where a mass field exists) |
| Constants | `pi`, `PI`, `Pi` |
| Variable ref | `#name` (identifier `[A-Za-z_][A-Za-z0-9_]*`, case-sensitive); callable if the variable holds a function: `#Adjust(20mm)` |
| Function call | `name(arg, ...)` (DOC-57) |
| Array literal / index | `[3,5,6,7][2]` -> 6 (0-based) |
| Arithmetic | `+ - * / ^ %` and unary `-` |
| Comparison | `> < >= <= == !=` (`>`, `<` documented; others inferred from FeatureScript) |
| Logical | `&&`, `||` (documented), `!` (inferred) |
| Conditional | `cond ? a : b` |
| Grouping | `( )` |

- **Behavior / semantics:**
  - Precedence (inferred, FeatureScript/C-like): postfix unit / call / index > `^` (right-assoc) > unary `- !` > `* / %` > `+ -` > relational > equality > `&&` > `||` > `?:`. Verify against FeatureScript operator docs.
  - No implicit multiplication: `(2*3)(1/3)` invalid; write `(2*3)*(1/3)`.
  - `%` modulo: `5%2` = 1.
  - Booleans from comparisons / configuration checkboxes (`#alwaysReinforce`) usable in ternaries.
  - Ternary example: `#width>5?7:4` -> 7 or 4 (in field's default unit); `(#width > 3in && #width < 5in) ? 3 : 2`.
- **Edge cases & errors:** see DOC-58, DOC-61.
- **OCCT mapping:** N/A. App: hand-written Pratt parser in Rust producing an AST; evaluator over `Value = Number{v, dims} | Bool | Array | Map | String | Function`.
- **Priority:** P0

### DOC-57 Expression functions and constants
- **Purpose:** Built-in functions available in numeric fields.
- **Sources:** H:Home/numeric_fields; FS: math.fs, units.fs.
- **Selection inputs:** none.
- **Parameters:**

| Function | Args -> result | Notes |
|---|---|---|
| `sin cos tan` | angle or unitless -> unitless | unitless argument interpreted as degrees in numeric fields (`sin(30)` = `sin(30 deg)` = 0.5); FS signature takes ValueWithUnits angle |
| `asin acos atan` | unitless -> angle | result in degrees display; divide by `deg` for unitless (`atan(1)/deg` = 45) |
| `atan2(y, x)` | unitless/unitless or length/length -> angle | `atan2(4, 5)` = polar angle of (5,4) |
| `sinh cosh tanh asinh acosh atanh` | unitless -> unitless | |
| `sqrt` | x -> x^(1/2) units | `sqrt(4 in^2)` = 2 in; `sqrt(2in * 3mm)` valid |
| `abs` | any -> same units | |
| `exp`, `log` (natural), `log10` | unitless -> unitless | |
| `ceil floor round` | unitless -> unitless | `ceil(5.667)`=6, `floor(5.667)`=5 |
| `min max` | (a, b) same units, or (array) | |
| Constant `pi` | 3.14159... | |
| Other FS math functions shown in autofill when `#` not typed: `exp2`, `hypot(a,b)`, `roundToPrecision(v, p)`, `clamp(v, lo, hi)`, `range`, `argMin`, `argMax` (math.fs exports; availability in fields inferred) | | |

- **Behavior / semantics:** Functions follow FeatureScript semantics on ValueWithUnits (unit dimensions tracked as exponent maps; e.g. `sqrt` halves exponents; trig requires angle).
- **Edge cases & errors:** `sin(30)/deg` invalid (1/deg). `sqrt(16m)` invalid (m^0.5). Unknown name -> PARAMETER_EXPRESSION_UNKNOWN_FUNCTION.
- **OCCT mapping:** N/A. App: Rust std f64 math; units as small integer exponent vector over (length, angle, mass, time, ...), conversion factors exactly as units.fs (inch = 0.0254 m, foot = 12 in, yard = 3 ft, degree = 0.0174532925199432957692 rad, gram = 0.001 kg, ounce = 28.349523 g, pound = 16 oz).
- **Priority:** P0

### DOC-58 Unit algebra and validity rules
- **Purpose:** Which expressions are accepted for a field.
- **Sources:** H:Home/numeric_fields.
- **Selection inputs:** none.
- **Parameters:** examples:

| Valid | Invalid |
|---|---|
| `3in+2.5in` | `3+2.5in` (unitless + unit) |
| `3mm+2.5in` (mixed length units fine) | `3mm+2deg` (length + angle) |
| `3+2` | `3in+2` |
| `(2*3)*(1/3)` | `(2*3)(1/3)` |
| `sqrt(16)m` | `sqrt(16m)` |
| `cos(30deg)` | `30°` (degree symbol) |
| `3mm+(2mm*3mm)/(6mm)` | `3in*3in` (in^2) |
| `sqrt(4 in^2)` | `sin(30)/deg` (1/deg) |
| | `3[u]+1[u]/2[u]` (unit over unit next to a separate unit) |

- **Behavior / semantics:**
  - Final result must be unitless or a single unit to the first power; intermediate products/quotients may have other powers if they cancel (`(2mm*3mm)/(6mm)`).
  - Do not mix dimension types in addition/subtraction; unitless + unit is an error (no implicit default unit inside sums).
  - Anything resulting in 1/[unit] or [unit]^2 is rejected.
  - Unitless result in a length/angle field is interpreted in the default unit (`3+2` -> 5 default units); unitless fields (counts, Rho, ratios) require unitless results.
  - Sketch dimension or feature field of type length requires length; angle field requires angle (unitless -> default angle unit).
- **Edge cases & errors:** NO_UNIT "Value is missing required units."; PARAMETER_EXPRESSION_VALIDATION_UNITS_MISMATCH_3ARGS (units mismatch); PARAMETER_EXPRESSION_VALIDATION_INCORRECT_TYPE.
- **OCCT mapping:** N/A. App: dimensional analysis in evaluator; final coercion step per field kind (Length, Angle, Real, Integer, Any).
- **Priority:** P0

### DOC-59 Variable references, scoping and autofill
- **Purpose:** Use named variables in any numeric field.
- **Sources:** H:PartStudio/variable, H:PartStudio/variable_table, H:VariableStudio/variable_studio, H:Home/numeric_fields; FS: variable.fs (`verifyVariableNameIsValid`, `setVariable`, `getVariable`, `makeLookupFunction`), partStudio.fs.
- **Selection inputs:** none.
- **Parameters:** variable name rules: regex `[a-zA-Z_][a-zA-Z_0-9]*`, case-sensitive, cannot start with a digit, max 10000 chars, must not equal an existing query-variable name.
- **Behavior / semantics:**
  - Reference syntax `#name` anywhere an expression is accepted (sketch dimensions, feature fields, variable values, mates). Display shows the evaluated value; editing shows the expression.
  - Sources, in effect order (later shadows earlier) (inferred from documented override rules): configuration inputs/configuration variables (set as context variables before any feature runs, per `definePartStudio`); Variable Studio tables published/inserted into the Variable table, then local Variable table entries (later table overrides earlier); Variable features in the Feature list (a feature-list variable with the same name overrides a table variable). Variables are features: a Variable feature is only visible to features after it ("Only variables that exist in the current context are shown; variables further down in the Feature list are not shown").
  - Lookup in generated Part Studio code: `lookup = (name) => try(getVariable(context, name))`; missing -> undefined -> error.
  - Changing a variable value regenerates all dependent features; renaming offers "Update all references" to rewrite every `#oldname`.
  - Autofill: opens on `#`, `_` or a letter; filters as you type; arrow keys + Enter, mouse wheel + click; Esc or blur closes without inserting; computed values shown in light gray (not inserted); FS functions included when `#` not typed; function argument placeholders not inserted; "New variable" entry opens Create variable dialog (Feature variable (default) or Configuration variable); new Variable feature inserted immediately before the feature being edited.
  - Feature list search `:variable name` shows features that use or change a variable.
  - Arrays + index variable pattern: `#diameter = [0.25, 0.5, 1][#config]`.
  - Variable of type Any can hold a FeatureScript function: `function(len) { return len * 2 + 2.5 mm; }` used as `#Adjust(20mm)`.
- **Edge cases & errors:** VARIABLE_NOT_FOUND "Variable not found, is suppressed, or has not been set."; PARAMETER_EXPRESSION_UNKNOWN_VARIABLE; VARIABLE_NAME_INVALID; VARIABLE_NAME_TOO_LONG; VARIABLE_NAME_ALREADY_USED_IN_QUERY_VARIABLE; VARIABLE_CANNOT_EVALUATE (warning). Suppressing a Variable feature makes dependents fail. Variables cannot be used in sketch patterns (CANNOT_USE_VARIABLES_IN_SKETCH_PATTERNS).
- **OCCT mapping:** N/A. App: during regeneration maintain an ordered environment (persistent map) snapshot per feature index; each feature's expressions evaluate against the environment at its position; record dependency edges (feature -> variable names) for invalidation.
- **Priority:** P0

### DOC-60 Advanced expression syntax (arrays, conditionals, functions)
- **Purpose:** Lookup tables and conditional values in fields.
- **Sources:** H:Home/numeric_fields ("Advanced syntax"), H:PartStudio/variable.
- **Selection inputs:** none.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Array literal + index (0-based): `[3,5,6,7][2]` = 6; `[3,5,6,7][3]` = 7; index may be an expression/variable (`[1, 4, 9, 16][#i]`).
  - Ternary with comparisons and logic: `#width>5?7:4`; `(#width > 3in && #width < 5in) ? 3 : 2`; `(#width > 3in || #alwaysReinforce) ? 3 : 2`.
  - Variables of type Any accept any FeatureScript value: boolean, map, array, string, function; numeric with any units.
  - Units in branches follow normal rules (both branches should yield compatible units for the field) (inferred).
- **Edge cases & errors:** Index out of range -> evaluation error (inferred). Comma-decimal locale requires spaces after array commas.
- **OCCT mapping:** N/A. App evaluator: full value types; for noshape a sandboxed subset (no loops) is sufficient unless a FeatureScript-compatible runtime is built.
- **Priority:** P1

### DOC-61 Expression errors and feedback
- **Purpose:** Error reporting for invalid expressions.
- **Sources:** H:Home/numeric_fields, H:Home/dialogs; FS: errorstringenum.gen.fs, expressionvalidationresult.gen.fs (`ExpressionValidationResult { VALID, NO_UNIT, ERROR }`).
- **Selection inputs:** none.
- **Parameters:** error enums: PARAMETER_EXPRESSION_VALIDATION_SYNTAX_ERROR_0ARGS ("Syntax error in expression."), PARAMETER_EXPRESSION_VALIDATION_UNITS_MISMATCH_3ARGS, PARAMETER_EXPRESSION_VALIDATION_RESERVED_ID_1ARG, PARAMETER_EXPRESSION_VALIDATION_CANNOT_APPLY_OP_TO_1ARG/2ARGS/3ARGS, PARAMETER_EXPRESSION_VALIDATION_INCORRECT_TYPE, PARAMETER_EXPRESSION_UNKNOWN_FUNCTION, PARAMETER_EXPRESSION_UNKNOWN_VARIABLE, EXPRESSION_FAILED_VALIDATION ("Expression failed validation."), NO_UNIT ("Value is missing required units."), VARIABLE_NOT_FOUND.
- **Behavior / semantics:** Validation runs as you type (three outcomes: valid, valid-but-missing-unit -> default unit applied, error). Error -> red outline, tooltip message, accept disabled; feature that later fails to evaluate (e.g. variable deleted) shows a regeneration error on the feature in the Feature list.
- **Edge cases & errors:** as listed.
- **OCCT mapping:** N/A.
- **Priority:** P0

### 1.6 Collaboration aspects relevant to the data model

### DOC-62 Comments (data-model aspects)
- **Purpose:** Threaded comments and tasks attached to a document workspace and optionally tagged to model entities.
- **Sources:** H:Collaboration/comments_on_workspaces.
- **Selection inputs:** Tag entity: features, sketches and sketch dimensions, tolerant dimensions/hole callouts, MBD inspection annotations, mate connectors (incl. implicit), mates, entities in subassemblies, parts/edges/faces.
- **Parameters:** text (with @mentions, auto-linked URLs; Onshape URLs rendered as document/version/tab links), image attachment, markup image, assignee (one of the mentioned users), resolved flag, notification opt-in; filters Entire document / This workspace only; All comments / All open tasks / Open tasks assigned to me / Completed tasks.
- **Behavior / semantics:**
  - Comments belong to a document and one workspace (set differs per workspace); not recorded in workspace history; follow an assembly moved to a new subassembly.
  - Comment icons on tagged entities/features visible only while the Comments panel is open; links in a comment navigate to the tab and highlight the entity.
  - Markup tool: arrows, lines, freeform, ellipse/circle, rectangle/square, text, color/size; Shift = 45 deg / square/circle, Ctrl = centered; Ctrl+Z undo within markup; saved as image attachment.
  - Permission: Edit, Comment and Can view allow commenting; versions have no Comments panel.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App: side table keyed by (document, workspace) with entity tags stored as persistent references (feature id / persistent topology id), outside the microversion DAG.
- **Priority:** P2

### DOC-63 Simultaneous editing, social cues, follow mode, non-collaborative tabs
- **Purpose:** Real-time multi-user editing in one workspace.
- **Sources:** H:Collaboration/collaboration, H:Collaboration/follow_mode.
- **Selection inputs:** double-click a collaborator's social-cue icon to follow; click anywhere to stop.
- **Parameters:** none.
- **Behavior / semantics:**
  - All changes appear in real time to all users in the workspace; social cues (colored initials) on tabs, features being edited, and Documents page details.
  - Follow mode mirrors: active tab and actions, cursor (hand icon), view/render modes incl. section view, graphics and Feature list selections, exploded views, feature edit dialogs (not creation), part moves and open sketches, Reference manager, Sketch text, Derived, Mass properties/Measure dialogs, right-side panels (explode, BOM, configuration, custom tables, appearance, hole table). One leader can have many followers; a follower follows one user.
  - Non-collaborative tabs (Drawings, third-party apps): one editor at a time; others see who holds it and can take over with "Open tab, close it for other user".
  - View-only collaborator moving the rollback bar stops receiving rollback-bar updates until reload (per-user rollback position is local).
- **Edge cases & errors:** as above.
- **OCCT mapping:** N/A. For noshape: optional; if implemented, sync microversion commits over a server with per-element operation streams; follow mode = broadcast of UI state.
- **Priority:** P2

### DOC-64 View-only mode
- **Purpose:** Open documents without risk of edits.
- **Sources:** H:Collaboration/view_only_mode, H:Document/document_menu.
- **Selection inputs:** none.
- **Parameters:** company preference "Always open documents in View only mode".
- **Behavior / semantics:** Banner "You are in View only mode" with Edit button; toolbars hidden; Document menu > Switch to view only mode returns. Creators must toggle manually even with the preference. Dialogs can be opened read-only by users without edit permission.
- **Edge cases & errors:** none.
- **OCCT mapping:** N/A. App: read-only flag on the open session (also used for versions).
- **Priority:** P2

### 1.7 Import into a document (overview)

### DOC-65 Import into document (overview; formats in section 7)
- **Purpose:** Bring external files into a new document (from Documents page) or into the current document (Insert new tab > Import).
- **Sources:** H:Document/importing_files, H:Document/working_with_imported_cad, H:File/supported_file_formats; FS: `importForeign` (importForeign.fs).
- **Selection inputs:** local files (multi-select, drag-drop onto Documents list), cloud drives (Google Drive, Dropbox, OneDrive).
- **Parameters (CAD processing options, overview):** Import to a single document / Split into multiple documents (one document per part/assembly option; folder named after file) / Combine to a single Part Studio (flatten); Import appearances (default on); Import material density; Orient imported models with Y Axis Up; Create a composite part when importing multiple or non-solid bodies; Join adjacent surfaces; IGES postprocessing; mesh units (cm, ft, in, m, mm, yd) for STL/OBJ; Allow import of parts with faults; owner (company users).
- **Behavior / semantics:**
  - From Documents page: new document named after the file, version created automatically; from within a document: new tab(s) named after the file, type shown in tab icon.
  - Every imported file becomes a File tab; translatable CAD creates Part Studio(s)/Assembly(ies) as needed; the Part Studio holds an editable Import feature (Import appearances, Source is 'Y Axis Up', Allow faulty parts, Create composite, ...) referencing the blob, so options can be changed post-import and the file can be replaced via Update.
  - Zip assemblies: zip name must equal the top-level assembly name, flat structure, no special characters.
  - Split into multiple documents: >100 unique parts at a level -> extra documents; links preserved; top-level assembly Download only, linked part documents Update and Download.
  - Faulty parts imported with red names; fractured bodies auto-combined into closed composite parts.
  - Upload limit 4 GB per file; progress/errors in Notifications import queue with retry.
- **Edge cases & errors:** Y Axis Up unsupported for .ipt and .catpart.
- **OCCT mapping:** STEPCAFControl_Reader / IGESCAFControl_Reader with XCAF for names, colors, assembly structure; RWStl, RWObj, RWGltf_CafReader, RWMesh for meshes; ShapeFix_Shape / ShapeUpgrade for faulty geometry; BRepBuilderAPI_Sewing for "Join adjacent surfaces". Details in section 7.
- **Priority:** P1

### 1.8 Proposed noshape data model (design suggestion)

### DOC-66 Immutable microversion DAG and content-addressed store (design suggestion)
- **Purpose:** Reproduce Onshape's "every edit persisted, any state restorable, versions immutable, branches and merges" semantics in a native app.
- **Sources:** derived from DOC-29..DOC-41; Onshape API terminology (documents/{did}/w|v|m/{id}/e/{eid}) (inferred, not in help corpus).
- **Selection inputs:** none.
- **Parameters:** core records (Rust sketch):

```text
DocumentId, BranchId, VersionId, MicroversionId, ElementId = 128-bit ids (ULID/UUIDv7)
Blob { hash: Blake3, bytes }                                  // content-addressed, immutable
ElementState { kind, name, schema_ver, payload_hash }          // payload = canonical serialization (DOC-68)
Microversion {
    id, parents: [MicroversionId; 1..2],                       // 2 parents = merge
    branch: BranchId, author, timestamp,
    elements: PersistentMap<ElementId, ElementStateHash>,      // whole-document snapshot by reference
    tab_tree_hash,                                             // tab order + tab folders
    workspace_settings_hash,                                   // units, precision, kernel_behavior_version
    message: { element: ElementId?, action: String, object: String } // "Part Studio 1 :: Insert feature : Fillet 1"
}
Version { id, name, description, microversion: MicroversionId, auto: bool, props }   // props mutable side table
Branch  { id, name, forked_from: VersionId, head: Option<MicroversionId>, protected: bool, deleted: bool }
Document { id, name, description, owner, branches, versions, default_branch, notes, thumbnail_elem }
```

- **Behavior / semantics:**
  - Every committed user action (feature accept, sketch close, tab create/rename/reorder/delete, property apply, units change, restore, merge, reference update) appends one Microversion to the branch head; unchanged element states are shared by hash, so snapshots are cheap.
  - Workspace = branch head pointer. Version = named pointer to a microversion (immutable). Branch from version = new branch whose first parent is the version's microversion.
  - Restore = new microversion copying selected element hashes from the target point (no history rewrite). Merge = microversion with two parents (DOC-70). "Revert merge" = restore to the first parent.
  - History list per branch = walk parents along the branch; Show changes paging (25) is a UI concern.
  - Element ids are stable across all microversions/branches/versions so references (intra-doc and external) remain valid; external references store (DocumentId, VersionId, ElementId) and resolution reads the target version's microversion.
  - "Tab changed since version X" = compare ElementStateHash between two microversions: O(1), drives blue update badges (DOC-46) and merge "tabs with/without changes".
  - Storage: one directory or SQLite file per document (`objects/` blobs + `refs` table); garbage collection unnecessary because history is permanent; optional pack/compression.
  - Regeneration cache keyed by (ElementStateHash, config, upstream reference hashes, kernel_behavior_version) -> tessellation + BRep (stored as OCCT BinTools/`.brep` blobs) so reopening any version is fast and deterministic.
  - Sync (optional, later): microversions are immutable objects, so replication = exchanging missing objects + fast-forwarding branch heads; concurrent edits on the same branch need server ordering or automatic per-element rebase.
- **Edge cases & errors:** Concurrent writers in a single-user desktop app: lock the document store (one process) or use SQLite WAL with optimistic head compare-and-swap.
- **OCCT mapping:** N/A (app data model, content-addressed store). OCCT only for cached shape serialization (BinTools_ShapeSet / BRepTools::Write).
- **Priority:** P0

### DOC-67 Element ids, entity ids and reference records (design suggestion)
- **Purpose:** Stable identifiers so that references survive renames, reorders, branching, copying and moves.
- **Sources:** derived from DOC-15, DOC-21, DOC-43, DOC-45; FS: `Id` (feature ids), `PartStudioData`, `ImageData`/`TableData`/`CADImportData`.
- **Selection inputs:** none.
- **Parameters:**

```text
ElementId                         // unique within document, stable forever (duplicate/paste -> new id)
FeatureId  = string like "F1a2b3" // unique within a Part Studio, stable across edits/branches; subfeature ids are paths (FeatureId + ".op1")
PartId     = persistent body id derived from creating feature id + topological naming
EntityRef  = { feature_scope, persistent_topo_name }           // for faces/edges/vertices selections
ElementRef = {
    document: Option<DocumentId>,        // None = same document
    target:   Workspace | Version(VersionId) | Microversion(MicroversionId),
    element:  ElementId,
    configuration: Map<String, Value>,
    parts:    Option<Vec<PartId>>,       // or a query
    pinned:   bool                       // excluded from Update all
}
```

- **Behavior / semantics:**
  - Same-document references default to `Workspace` (live); cross-document references must be `Version` (enforce at creation; error otherwise, mirroring REST_ASSEMBLY_EXTERNAL_REFERENCE_REQUIRES_VERSION).
  - Move to document rewrites `ElementRef{document: None, Workspace}` to `{document: Some(dest), Version(v_auto)}`.
  - Paste tab into another document: rewrite intra-document refs of the pasted element to external refs pinned to the source document's current microversion/version (DOC-20), or drop with warning.
  - Self-reference / cycles: build an element dependency graph per microversion and reject cycles (Derived A->B->A).
  - Feature ids must never be reused after deletion so merge and compare can match features across branches.
- **Edge cases & errors:** Copy workspace (DOC-10): preserve ElementIds and FeatureIds inside the new document so internal refs remain valid; new DocumentId.
- **OCCT mapping:** Persistent topological naming: OCCT TNaming/TDF (OCAF) or a custom history-based naming scheme fed by BRepAlgoAPI/BRepBuilderAPI `Modified()/Generated()/IsDeleted()` histories (BRepTools_History). Custom work required; OCAF TNaming is fragile for complex models.
- **Priority:** P0

### DOC-68 Feature list serialization (design suggestion)
- **Purpose:** Canonical, diffable, mergeable on-disk form of a Part Studio (and analogous for Assemblies).
- **Sources:** derived from FS `definePartStudio` (partStudio.fs), feature preconditions (extracted/features/*.txt), DOC-37/DOC-38 merge and compare granularity.
- **Selection inputs:** none.
- **Parameters:** Part Studio payload (schema versioned):

```text
PartStudioState {
  schema_ver: u32,
  kernel_behavior_version: u32,         // analogue of FeatureScriptVersionNumber pin (DOC-41)
  features: Vec<Feature>,               // ordered; order is semantic
  rollback_index: per-user, NOT stored  // rollback bar position is local UI state
  configuration: Vec<ConfigInput>,      // list/checkbox/variable inputs with FS-style ids and defaults
  variable_tables: Vec<ElementRef | LocalVariableTable>,
  part_props: Map<PartId, PropertyMap>, appearances, materials,
  folders: Vec<FeatureFolder>           // Feature list folders
}
Feature {
  id: FeatureId, type_name: "extrude" | "fillet" | ..., name: String, suppressed: SuppressionExpr,
  params: Map<String /* FS param id, e.g. "depth" */, ParamValue>,
  feature_ver: u32
}
ParamValue = Expr { text: String, unit_kind } | Bool | Enum(String) | Query(Vec<EntityRef>) | String
           | ElementRef | Array(Vec<Map<String,ParamValue>>) | SketchData {...}
```

- **Behavior / semantics:**
  - Parameter ids and enum names match the FeatureScript precondition ids (e.g. `operationType`, `endBound`, `depth`) so the spec in sections 3+ maps 1:1 onto storage.
  - Numeric params store the user's expression text verbatim (e.g. `"#width/2 + 3 mm"`); evaluated SI value is cached but not authoritative. Display uses workspace units (DOC-54).
  - Canonical encoding: deterministic key order (BTreeMap), stable float formatting; JSON or CBOR; hash = Blake3 of canonical bytes (DOC-66).
  - Regeneration = fold over `features` with environment (variables, configuration) and context; equivalent to FS `definePartStudio`: set configuration values as variables, add default features (origin, Top/Front/Right planes) using default length unit, then execute features in order.
  - Sketches stored as a feature with entities, constraints, dimensions (expressions) and solver hints (last solved positions) to keep solutions stable.
  - Migrations: `schema_ver` and `feature_ver` upgrade functions; old `kernel_behavior_version` keeps old regeneration semantics until "Update workspace".
- **Edge cases & errors:** Unknown feature type (e.g. from newer app version) -> load as opaque, show error feature, preserve on save. Mirrors FEATURE_BAD_SERIALIZATION_VERSION / FEATURE_WRONG_SERIALIZATION_VERSION.
- **OCCT mapping:** N/A (serialization). Cached results in OCCT BRep binary format.
- **Priority:** P0

### DOC-69 Undo/redo implementation over microversions (design suggestion)
- **Purpose:** Implement Onshape undo semantics (per user, per tab, per session; dialog-scoped steps) on top of the immutable history.
- **Sources:** derived from DOC-31.
- **Selection inputs:** none.
- **Parameters:** `UndoEntry { element: ElementId, before: ElementStateHash, after: ElementStateHash, op: OpDescriptor, microversion: MicroversionId }`; stacks keyed by (session, user, element).
- **Behavior / semantics:**
  - While a dialog/sketch is open, keep a transient in-memory edit stack (not committed); on accept, commit one microversion and push one UndoEntry; on cancel, discard the transient stack.
  - Undo: if the element's current hash == entry.after (no intervening edits by others), commit a new microversion setting the element back to entry.before (message "Undo: <action>"); push to redo stack. If intervening edits exist, apply the inverse op (e.g. delete inserted feature id, restore edited feature's previous params) as a three-way merge of before/after against current; on conflict, refuse with message.
  - Redo symmetric; any new user action clears that element's redo stack.
  - Tab-level operations (create/delete/rename/reorder tab) go on a document-scope stack (inferred; Onshape help only says per tab).
  - Merge, restore and reference "Update all" are not on the undo stack; offer explicit revert (restore to pre-operation microversion) as Onshape does for merges.
- **Edge cases & errors:** Session end (close window) drops stacks; history remains.
- **OCCT mapping:** N/A (app data model).
- **Priority:** P0

### DOC-70 Merge algorithm (design suggestion)
- **Purpose:** Implement per-tab Keep / Merge / Replace with feature-level three-way merge for Part Studios and instance-level merge for Assemblies.
- **Sources:** derived from DOC-38, DOC-39.
- **Selection inputs:** none.
- **Parameters:** base = lowest common ancestor microversion, or the last merge point from the same source branch into this target if not reverted; per-element strategy map.
- **Behavior / semantics:**
  - Element set: union of element ids in base/source/target; created/deleted elements allow only Keep or Replace; unchanged-in-source elements default to Keep.
  - Part Studio merge (when both changed): three-way merge of `features` keyed by FeatureId: insertions from both sides kept (ordering: place source insertions after their predecessor feature from source; ties -> target first); deletions applied; parameter-level merge per feature (param id granularity; same param changed differently on both sides -> conflict, default to target, flag). Configuration inputs, materials, appearances merged by key. Result regenerated; failures shown as normal feature errors (Onshape warns merges "can be incompatible, causing errors").
  - Assembly merge: instances, mates, mate connectors, groups, configurations keyed by id with the same rules.
  - Drawings/blobs: Keep or Replace only.
  - Commit: microversion with parents [target_head, source_point]; record merge line metadata.
  - Preconditions: same `kernel_behavior_version` on both sides (else offer update), target not protected or source up to date with protected target, no other open sessions writing the document (desktop: file lock).
- **Edge cases & errors:** Feature references (queries) into features deleted by the other side -> broken references -> repair flow (DOC-42).
- **OCCT mapping:** N/A (app data model).
- **Priority:** P2
