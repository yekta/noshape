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
| Surfaces audited | 16 |
| Gaps found | 7 |
| Gaps fixed | 5 |

## Surfaces

| # | Surface | Live source | Gaps |
|---|---|---|---|
| S1 | In-document + (create) menu | menu-plus-tab.txt | 0 (CAM / PCB / Render Studio are paid apps) |
| S2 | Create version dialog | dialog-create-version.txt, behaviours (c) | D2 |
| S3 | ☰ document menu on a workspace | behaviours "☰ document menu" | 0 |
| S4 | ☰ document menu on a version | navbar-version-v1.txt | D3 |
| S5 | Version / workspace Properties modal | version-properties-dialog.txt, branch-edit-properties.txt, create-version-edit-properties.txt | D10 |
| S6 | Merge dialog | ui2-merge-dialog.html | 0 ("Keep Main" is composed from Keep + the target chip) |
| S7 | Documents page › Versions and history panel | documents-versions-history-panel.txt | D6 |
| S8 | Tab right-click menus, 12 kinds × active / inactive | ui2-tab-menus.json | 0 (Export… is dx's) |
| S9 | Tab manager panel, list view, sort menu, footer | panel-tab-manager*.txt, tab-manager-footer.json | 0 |
| S10 | Delete tabs dialog | ticket-delete-tab-dialog.txt | 0 |
| S11 | Documents page advanced search panel | documents-advanced-search.txt, documents-history-search-filter.txt | 0 |
| S12 | Documents page search results | documents-adv-search*.json\|txt, behaviours "Advanced search: empty state and results" | D7, D8 |
| S13 | Documents page create menu, row menu, folder sort, add column, empty trash, right panel (details, properties, notes) | documents-create-menu.txt, documents-row-context-menu.txt, documents-folder-sort-menu.txt, documents-add-column-menu.txt, documents-empty-trash-modal.txt, documents-right-panel-*.txt, documents-details-panel.txt | 0 |
| S14 | Standard content browser | skb-standard-content*.json, ui2-standard-content*.json\|html | D9 |
| S15 | Insert dialog, other documents | assembly-insert-other-*.txt | D9 |
| S16 | Part / assembly / instance Properties dialogs | dialog-*-properties.txt | 0 |

## Gaps

| # | P | Gap | State |
|---|---|---|---|
| D2 | P2 | Create version: live warns "These drawings should be updated before creating a version:" with drawing links and an "Update drawings." button. Ours has no warning. It needs a drawing staleness query from the drawings side (dx). | open: asked dx in the handoff |
| D3 | P1 | ☰ on a version (a version window): live shows Rename document, Move to…, Restore deleted workspaces… · Copy version…, Version properties… · Print… · Close document. Ours showed the workspace items (Copy workspace…, Update workspace…, Workspace units…, Workspace properties…, Switch to view only mode). | fixed: the version variant; Copy version… makes a new document from the version's state (`start_library::copy_version_named`) |
| D6 | P2 | Documents page › Versions and history: live rows have a 140 px graph column (white workspace ring, blue version dots), "Show changes…" under Main and the versions, a selected row (light blue) and the tooltip "No description" / "Description: …". Ours lists Name / Modified only. | open |
| D7 | P3 | Documents page while searching: live's header is "Search results in" with the place in blue / 600. With no results it shows "No results found that match this search. Try other search terms or refine the search by clicking the arrow in the search box." Ours kept the place header and said "No documents match …". | fixed |
| D8 | P1 | Documents page search results with Type Part (Part Studio, …): live lists **each part (tab) as its own row**, with thumbnail, part icon, bold matched name, ⚲ Main, a state badge, and the Part number and Revision columns. Ours filtered whole documents. | fixed: a search of parts or tab types lists a row per item (`start_search::hits`), with the item's icon, the searched text in bold, the workspace, the part's state, Part number and Revision; a double click opens the document at its tab. Part names come from regeneration, so each workspace keeps an index of its Part Studios' parts (store meta `search_parts:<workspace>`, `app-core/search_index.rs`), written with the thumbnail. A document not edited since has none and lists only its named parts, as do versions. |
| D9 | P3 | Search boxes of the standard content browser and Insert › Other documents: tooltip "Type and press enter to search", and the clear button's tooltip "Clear search". | open |
| D10 | P2 | ☰ › Workspace properties…: live runs the same command as the version's Version properties… (the metadata modal with the tab tree). Ours opened a plain name / description modal. | fixed: opens history-ui's Properties modal for the workspace |
