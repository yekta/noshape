| DOC-01 | Document (container concept) | P0 | App data model: document = root of content-addressed store |
| DOC-02 | Create document | P0 | App data model: new store, initial microversion with Part Studio 1 + Assembly 1, Start version, Main branch |
| DOC-03 | Ownership, sharing, permissions, public/private | P2 | Optional sync-server ACL / file-system permissions |
| DOC-04 | Documents page (document browser) | P1 | GPUI list/grid over local library index (SQLite) + thumbnail cache |
| DOC-05 | Folders | P1 | Library index tree or file-system directories |
| DOC-06 | Labels | P2 | Tag table in library index |
| DOC-07 | Built-in filters | P2 | Saved queries over library index |
| DOC-08 | Search and advanced search | P2 | Full-text index (e.g. tantivy) over document/element/part metadata |
| DOC-09 | Trash, restore, permanent delete | P1 | Trash directory + retention purge |
| DOC-10 | Copy workspace / Copy version | P1 | Snapshot microversion element map into new store (preserve element/feature ids) |
| DOC-11 | Document toolbar and Document menu | P1 | App UI (GPUI) |
| DOC-12 | Properties and metadata | P1 | Property maps in element state; version metadata side table |
| DOC-13 | Document notes | P2 | Markdown blob on document + Markdown renderer |
| DOC-14 | Document panel | P1 | App UI dock |
| DOC-15 | Tab (element) types | P0 | ElementKind enum with per-kind serialized state |
| DOC-16 | Tab bar and active-tab semantics | P0 | Tab order in workspace state; active tab in local session state |
| DOC-17 | Create tab | P0 | Command -> new microversion with new ElementId |
| DOC-18 | Rename tab | P0 | Element metadata edit (refs are id-based) |
| DOC-19 | Duplicate tab | P0 | Deep copy element state with new ElementId |
| DOC-20 | Copy to clipboard / Paste tab | P1 | Clipboard payload = serialized element + source doc/microversion; rewrite refs |
| DOC-21 | Move to document | P1 | Two-store transaction; rewrite refs to version references |
| DOC-22 | Delete tab | P0 | Reverse-reference index for dependent warning; history keeps state |
| DOC-23 | Reorder tabs | P0 | Ordered element list (fractional indexing) |
| DOC-24 | Tab folders | P1 | Folder tree in workspace state |
| DOC-25 | Tab manager panel | P1 | App UI |
| DOC-26 | Tab context menu (workspace vs version) | P0 | App UI; command availability by read-only flag |
| DOC-27 | File (blob) tabs | P1 | Blob store; translators via STEPControl/IGESControl/RWStl/RWObj/RWGltf/XCAF |
| DOC-28 | Select as document thumbnail | P2 | Offscreen render to PNG |
| DOC-29 | Workspace | P0 | Branch head pointer to latest microversion |
| DOC-30 | Autosave and persistent change history | P0 | Append-only microversion log; commit per completed action |
| DOC-31 | Undo / Redo | P0 | Per (user, element, session) stacks over microversions (see DOC-69) |
| DOC-32 | Versions | P1 | Named immutable pointer to microversion + mutable metadata |
| DOC-33 | Branches, create/delete/restore workspaces | P1 | Branch records {fork version, head, deleted flag} |
| DOC-34 | Versions and history panel (version graph) | P1 | DAG lane layout UI over microversion graph |
| DOC-35 | Viewing a past state / opening a version | P1 | Read-only session at microversion; regen cache |
| DOC-36 | Restore to a point in history | P1 | New microversion copying selected element hashes |
| DOC-37 | Compare | P2 | Feature-id matched list diff; dual regen overlay (optional BRepAlgoAPI_Cut highlight) |
| DOC-38 | Merge | P2 | Per-element three-way merge (see DOC-70) |
| DOC-39 | Merge blockers and conflict rules | P2 | Precondition checks (behavior version, protection, document lock) |
| DOC-40 | Workspace protections | P2 | Branch flag enforced by command layer |
| DOC-41 | Update workspace (library/behavior version) | P1 | kernel_behavior_version pin per workspace; gated feature behavior |
| DOC-42 | Repair: View in repair, Edit healthy moment, Replace reference | P1 | Dual-context UI + persistent naming history (BRepTools_History / custom TNaming) |
| DOC-43 | Linked documents (version-based references) | P1 | ElementRef {doc, version, element, config}; cross-store resolution + shape cache |
| DOC-44 | Insert from other documents | P1 | Picker UI + reference creation; inline create-version |
| DOC-45 | Derived feature linking/update model | P1 | Regenerate source element; BRepBuilderAPI_Copy + BRepBuilderAPI_Transform placement |
| DOC-46 | Update notifications and Reference manager | P1 | Reference index; element-hash comparison across versions; recursive auto-versioning |
| DOC-47 | Change to version / Update linked document | P1 | ElementRef target switch |
| DOC-48 | Open linked document and link errors | P1 | Navigation + error states |
| DOC-49 | Linked custom features / Feature Studio imports | P2 | Future plugin/scripting system with versioned modules |
| DOC-50 | Where used | P2 | Global reverse index of version references |
| DOC-51 | Publications | P2 | Special document kind with external references only |
| DOC-52 | Structure view | P2 | Release-management dependent tree view (out of core scope) |
| DOC-53 | Variable Studios / Variable tables as variable sources | P1 | Variable scope chain; ElementRef to Variable Studio versions |
| DOC-54 | Workspace default units | P0 | SI internal storage + per-workspace parse/display units; per-unit default values |
| DOC-55 | Numeric field entry and display | P0 | GPUI numeric input (scroll increments, expression vs value display) |
| DOC-56 | Expression language grammar | P0 | Rust Pratt parser + AST evaluator |
| DOC-57 | Expression functions and constants | P0 | f64 math with unit-dimension vectors; units.fs conversion factors |
| DOC-58 | Unit algebra and validity rules | P0 | Dimensional analysis + per-field coercion |
| DOC-59 | Variable references, scoping, autofill | P0 | Ordered environment per feature index + dependency edges |
| DOC-60 | Advanced expression syntax (arrays, ternary, functions) | P1 | Evaluator value types (array/map/bool/function) |
| DOC-61 | Expression errors and feedback | P0 | Validation result enum (VALID/NO_UNIT/ERROR) + messages |
| DOC-62 | Comments | P2 | Side table keyed by (document, workspace) with persistent entity tags |
| DOC-63 | Simultaneous editing, social cues, follow mode | P2 | Optional sync server; broadcast UI state |
| DOC-64 | View-only mode | P2 | Session read-only flag |
| DOC-65 | Import into document (overview) | P1 | STEPCAFControl/IGESCAFControl/XCAF, RWStl/RWObj/RWGltf, ShapeFix, BRepBuilderAPI_Sewing (section 7) |
| DOC-66 | Immutable microversion DAG + content-addressed store (design suggestion) | P0 | App data model (Blake3 content-addressed objects, branch heads, versions) |
| DOC-67 | Element ids, entity ids, reference records (design suggestion) | P0 | Stable ULIDs + persistent topological naming (BRepTools_History / custom) |
| DOC-68 | Feature list serialization (design suggestion) | P0 | Canonical JSON/CBOR keyed by FS param ids; schema/behavior versions |
| DOC-69 | Undo/redo over microversions (design suggestion) | P0 | App data model (undo entries with before/after element hashes) |
| DOC-70 | Merge algorithm (design suggestion) | P2 | App data model (three-way merge by element id, feature id, param id) |
