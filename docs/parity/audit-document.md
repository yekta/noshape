# Audit: the document-level surfaces of live against ours

User request 11-01: after the Sketch / Assembly / Sheet metal audit (audit-sketch-assembly.md), audit the document-level surfaces whole.
That covers:
- the Documents page (lists, filters, search, create menus; not sharing);
- the tab bar and tab menus;
- Versions and history (create version, branch, merge, compare, restore);
- Release management;
- the Insert dialogs (Insert part / assembly / from other documents, the standard content browser);
- Properties, Custom properties and Metadata.

Import / export and drawings belong to dx and are left out.

Sources: live DOM dumps in /root/onshape-research/live/dom/, behaviours.md, and whole-surface live requests.

Method: every label, tooltip, option and menu item of the live dump is checked against our sources with /root/skasm-scratch/audit/extract.py. Order and states are then checked by reading the builder code next to the dump.

These are not counted as gaps:
- strings that only name document objects (document, tab and part names, dates, standard content catalogue rows; the catalogue lives in model-data's JSON);
- cloud and plan items (sharing, subscriptions, Upgrade badges, CAM / PCB / Render Studio).

Priorities:
- P1: a missing tool, option or menu command;
- P2: a missing indicator or a wrong state;
- P3: looks and texts (tooltips).

## Counts

| | |
|---|---|
| Surfaces audited | 25 |
| Gaps found | 15 |
| Gaps fixed | 15 |

## Surfaces

| # | Surface | Live source | Gaps |
|---|---|---|---|
| S1 | In-document + (create) menu | menu-plus-tab.txt | 0 (CAM / PCB / Render Studio are paid apps) |
| S2 | Create version dialog | dialog-create-version.txt, behaviours (c) | D2 | P2 | Create version: live warns "These drawings should be updated before creating a version:" with the drawings' names and a drawing-update button (tooltip "Update drawings."). Ours had no warning. | fixed: `history_panel::stale_drawings` (ops' `drawing::old_state`, which the drawing's own update icon uses); the button marks them updated as the drawing's Update does, and the box goes away |
| S3 | ☰ document menu on a workspace | behaviours "☰ document menu" | 0 |
| S4 | ☰ document menu on a version | navbar-version-v1.txt | D3 |
| S5 | Version / workspace Properties modal | version-properties-dialog.txt, branch-edit-properties.txt, create-version-edit-properties.txt | D10 |
| S6 | Merge dialog | ui2-merge-dialog.html | 0 ("Keep Main" is composed from Keep + the target chip) |
| S7 | Documents page › Versions and history panel | documents-versions-history-panel.txt | D6 |
| S8 | Tab right-click menus, 12 kinds × active / inactive | ui2-tab-menus.json | 0 (Export… is dx's) |
| S9 | Tab bar, tab manager panel, list view, sort menu, footer | tabbar-1280.html, tabbar-scroll.json, panel-tab-manager*.txt, tab-manager-footer.json | D12 |
| S10 | Delete tabs dialog | ticket-delete-tab-dialog.txt | 0 |
| S11 | Documents page advanced search panel | documents-advanced-search.txt, documents-history-search-filter.txt | 0 |
| S12 | Documents page search results | documents-adv-search*.json\|txt, behaviours "Advanced search: empty state and results" | D7, D8, D13 |
| S13 | Documents page create menu, row menu, folder sort, add column, empty trash, right panel (details, properties, notes) | documents-create-menu.txt, documents-row-context-menu.txt, documents-folder-sort-menu.txt, documents-add-column-menu.txt, documents-empty-trash-modal.txt, documents-right-panel-*.txt, documents-details-panel.txt | 0 |
| S14 | Standard content browser | skb-standard-content*.json, ui2-standard-content*.json\|html | D9 |
| S15 | Insert dialog, other documents | assembly-insert-other-*.txt | D9 |
| S16 | Part / assembly / instance Properties dialogs | dialog-*-properties.txt | 0 |
| S17 | Compare (bar, tab states, feature and parameter rows, moved markers) | ui2-compare*.json\|html | 0 |
| S18 | Release management | upsell-release-management-modal.txt (the free plan shows only the upsell), help enterprise_settings_release_management | not auditable on live; ours follows the help |
| S19 | Configured part properties panel (tables, menus, cell editors) | cp-*.json\|html | D11 |
| S20 | In-document Versions and history: row menus (workspace, version, change), Create version / Branch dialogs, Restore | behaviours 68, 1733, 2625, 3317-3351 | D14 |
| S21 | Insert dialog: other documents, configured, versions banner ("N changes since the last version.") | assembly-insert-*.txt | 0 |
| S22 | Workspace protection dialog, banner, blocked edits, merge | ui2-workspace-protection-*.json\|html | D15 |
| S23 | Tab › Properties… on every tab kind | live 1738, dom/skasm-1738-tab-properties.txt | D16 |
| S24 | ☰ › Document details… | live 1738, dom/skasm-1738-document-details.txt | D17 |
| S25 | ☰ › Copy workspace… / Copy version… | live 1738, dom/skasm-1738-copy-dialogs.txt | D18 |

## Gaps

| # | P | Gap | State |
|---|---|---|---|
| D2 | P2 | Create version: live warns "These drawings should be updated before creating a version:" with the drawings' names and a drawing-update button (tooltip "Update drawings."). Ours had no warning. | fixed: `history_panel::stale_drawings` (ops' `drawing::old_state`, which the drawing's own update icon uses); the button marks them updated as the drawing's Update does, and the box goes away | P2 | Create version: live warns "These drawings should be updated before creating a version:" with drawing links and an "Update drawings." button. Ours has no warning. It needs a drawing staleness query from the drawings side (dx). | open: asked dx in the handoff |
| D3 | P1 | ☰ on a version (a version window): live shows Rename document, Move to…, Restore deleted workspaces… · Copy version…, Version properties… · Print… · Close document. Ours showed the workspace items (Copy workspace…, Update workspace…, Workspace units…, Workspace properties…, Switch to view only mode). | fixed: the version variant; Copy version… makes a new document from the version's state (`start_library::copy_version_named`) |
| D6 | P2 | Documents page › Versions and history: live rows have a 140 px graph column (white workspace ring, blue version dots), "Show changes…" under Main and the versions, a selected row (light blue) and the tooltip "No description" / "Description: …". Ours listed Name / Modified only, without the search box, the toggles or the legend. | fixed: graph column (a lane per workspace, a branch drawn from the version it was made from), Show / Hide changes… with the changes since the version before (description, author, time), the selected row, the description tooltip (`start_library::HistoryRow`); the panel's name row, Search history box, Hide merges / Hide automatic versions toggles and legend |
| D7 | P3 | Documents page while searching: live's header is "Search results in" with the place in blue / 600. With no results it shows "No results found that match this search. Try other search terms or refine the search by clicking the arrow in the search box." Ours kept the place header and said "No documents match …". | fixed |
| D8 | P1 | Documents page search results with Type Part (Part Studio, …): live lists **each part (tab) as its own row**, with thumbnail, part icon, bold matched name, ⚲ Main, a state badge, and the Part number and Revision columns. Ours filtered whole documents. | fixed: a search of parts or tab types lists a row per item (`start_search::hits`), with the item's icon, the searched text in bold, the workspace, the part's state, Part number and Revision; a double click opens the document at its tab. Part names come from regeneration, so each workspace keeps an index of its Part Studios' parts (store meta `search_parts:<workspace>`, `app-core/search_index.rs`), written with the thumbnail. A document not edited since has none and lists only its named parts, as do versions. |
| D9 | P3 | Search boxes of the standard content browser and Insert › Other documents: tooltip "Type and press enter to search", and the clear button's tooltip "Clear search". | fixed (the clear button's tooltip landed with audit G7) |
| D10 | P2 | ☰ › Workspace properties…: live runs the same command as the version's Version properties… (the metadata modal with the tab tree). Ours opened a plain name / description modal. | fixed: opens history-ui's Properties modal for the workspace |
| D11 | P3 | Configured part properties: Add property ▾ has the tooltip "Configure a property of <part>". | fixed |
| D12 | P2 | Tab bar: a tab that references an older version of another document has a blue out-of-date icon, tooltip "Newer versions of linked documents available". The references can be Derived, an inserted Variable Studio, Assembly instances or Drawing views. Ours had none. | fixed: `workspace_update::version_refs` / `behind_tabs`, checked off the UI thread when the references change and when the window comes back |
| D13 | P2 | Documents search box: running an advanced search writes the query into the box as text (`type:part name:pvz`, `type:part,document`). Enter on such a text runs it, and clearing the box ends the search. Ours kept the box and the panel apart. | fixed: `Query::to_text` / `Query::parse_text` (type, name, description, partNumber, revision, state) |
| D14 | P1 | Versions and history row menus: live has "Open in new browser tab" on version, change and workspace rows. Ours left it out as web-only, though the tab menu's already opens another window. | fixed: version and change rows open a read-only window at that point; the current workspace's row opens another window on it. Other workspaces' rows still have none, because a second editable session on another branch isn't supported. Copy link stays out (web-only). |
| D15 | P2 | Workspace protection dialog: with drawings out of date, live shows "These tabs should be updated: <drawings>" above the checkbox and disables the checkbox. | fixed (for the current workspace) |
| D16 | P2 | Tab › Properties…: live opens the metadata modal on every tab kind, each with its own fields. Part Studio has Name, Description, Category "Onshape Part Studio", Title 1-3 and Not revision managed. Variable Studio adds Part number, Revision and State. Feature Studio has only Name, Description and Category. Files have the Vendor / Project / Product line rows. Assembly has the full set. Ours showed a name / description modal (drawings already had the modal). | fixed: properties_dialog picks the rows by category. An Assembly tab opens the assembly's own modal (`assembly_view::open_assembly_metadata`). Other tabs keep their metadata in store meta (tab_description / tab_part_number / tab_revision / tab_not_revision_managed / tab_item_meta). The Part Studio's left tree (P3) is still open. |
| D17 | P2 | ☰ › Document details…: live has "Enter a document description:" (Maximum of 10000 characters), a textarea and ☐ Not revision managed, with [Update] pale until changed and [Cancel]. Ours had Name, Owner and Created / Modified rows and [Save]. | fixed |
| D18 | P3 | Copy workspace… / Copy version…: live's titles are "Copy the active workspace (version) to a new document". Document name defaults to "<name> - Copy" (selected), with "Copy document to" and a folder browser. Ours was a "Copy of <name>" name box copied next to the document. | fixed (the button says "Create document"; live's "Create public document" is the Free plan's) |
