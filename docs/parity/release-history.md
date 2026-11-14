# Release management and history (studios, from 2026-11-14)

Coverage of crates/release-ui and crates/history-ui against Onshape.

- **History panel** (Versions and history, compare, merge, workspaces, version properties): ui-parity `ver-*`
  takes, 134 / 137 scenarios, 681 / 685 states (vibe6, 2026-11-14). Misses: ver-vrow-releases (live's take is the
  Free plan's upsell; re-record asked), ver-filter-tab (the start document's history: ours commits each sketch
  constraint, live's start has one "Add feature" per feature), ver-linked-selective-update (live's take missed the
  checkbox; re-recorded 11-14).
- **Release management**: live's account is on Professional since 2026-11-14 (before, Release… opened only the upsell).
  The live matrix below is live2's capture (25 rows: = 22, ? 3); the help matrix keeps the rows live hasn't shown yet
  (138: = 108, 17 settled by the live matrix, ≠ 2, missing 2, ? 2, cloud 7).
  Rows: 138; = 97, ≠ 20, missing 9, ? 6, cloud 6 (left out: other users, notifications).

## Live (Professional company, live2 2026-11-14: live/feel/release.json, feel/release/*.png)

| Item | Live | Ours | Test | Status |
|---|---|---|---|---|
| New company default | Enable managed workflows off; the settings page shows only it | `ReleaseSettings::default` (managed off) | ops old_settings_files_keep_their_values_and_get_the_defaults | = |
| Workflows off | no Release… (part, tab, instance), no Revision history…, no Releases on Versions rows, no Create release candidate icon; nothing greyed, no upsell | `managed_workflows()` gates them; release_create / release_obsolete refuse | ui-parity part-menu-release-workflows-off, ver-vrow-releases-workflows-off (queued) | = |
| Part Properties, workflows off | Part number, Revision, State dropdown (In progress … Discarded) by hand, Not revision managed; no release icon | the same | – | = |
| Defaults when turned on | Alphabetical, suffix *, Access 1st on, Require a note on, "Drawing has a pending update" on, other documents off, Do not allow creator greyed until Require an approver | the same defaults; Do not allow creator greyed until Require an approver (`release_prefs`) | ops settings test | = |
| Dialog title | "Create Release candidate" + refresh; a reopened draft "Review Release"; submitted / released "Review Release: <name>" + link icon | the same titles; the link icon (Copy release candidate URL) is a cloud link | release-ui the_release_dialogs_titles_are_lives | = (link: cloud) |
| Header | "N item(s) needs attention" + red ⓘ, ⋯ (View audit log, View workflow) | the same | – | = |
| Item row | thumbnail, icon + name, kind, yellow flag, red × "Remove from release" (top-level only) | the same ("Remove from release") | – | = |
| Item name click | nothing (dialog stays, no tab) | nothing | – | = |
| Columns | Revision (A, after a release "A ⟶ [B]"), State, Part number (red outline empty), Edit properties / Export… | the same | – | = |
| Revision too low | yellow band "The next available revision for part number LV2-100 is B." | the same text in a yellow band (`Issue::RevisionTooLow`) | model-core releases_obsolete_and_validate | = |
| Form | Release name *, Release notes * (Require a note), Approvers (ⓘ "Required for action Submit", no *), Observers, both "Search users or teams" | the same | – | = |
| Footer | undo / redo, Save draft (▴ Save draft / Delete draft on a draft), hint, Apply (blue), Submit (blue, needs approvers: "Submit requires Approvers"), Release (green #028502, #a2db7d disabled), Close | the same | – | = |
| Release | toast "Your action on <name> is being processed." | the same toast (DocEvent::Info) | – | = |
| Submit | toast "… View the status of Releases in Action items." | the same toast | – | = |
| Version graph | released ▲; Pending and Rejected open △ | the same | model-core pending_and_rejected_candidates_mark_their_version | = |
| Versions row Releases ▸ | only on a version with candidates; state badges Pending #769fcc, Rejected #bd3039 | only with candidates; badge colours not compared | ui-parity ver-vrow-releases (re-recorded) | ? |
| Review (approver) | Comment * required, Apply, Release, Reject ▴ (Reject / Discard), Close; Reject as creator confirms "Reject and override other approvers" | the same: Comment * with (comment history), Apply, Release, Reject ▴ (Reject / Discard), the creator's override confirm | – | = |
| Released review | no Release name field, notes read-only, Comment (comment history); Apply greyed, Clone (blue, "Recreate Release from the current workspace"), Close | the same | – | = |
| Drafts | Release drafts panel (thumbnail, name, "by <user> <time>", "Double click to open a draft") | the same | – | = |
| Revision history | heading part number; ▲ black / obsolete / auto-obsolete icon, letter, user, release name (bold), time, Version ↗ (Open in new tab), Document, Type; View release, Export, Obsolete revision (red) or View obsoletion | the same layout (our own icons) | – | = |
| Auto-obsolete | A's icon auto-obsolete, View obsoletion; B keeps Obsolete revision | the same | – | = |
| Obsolete candidate | Add to Obsoletion…, read-only row, Obsoletion name *, notes *, Approvers, Observers, re-releasable; Apply, Submit, Obsolete (red), Close | the same footer (Apply greyed) | ops release_parts_and_revision_history | = |
| Admin without required approvers | Release stays on; confirm "Release and override company policy" | the same | – | = |
| Barred creator | not offered as approver; no asterisk | the same | – | = |
| Assembly release | the assembly, its parts once (mirror adds none), linked parts In progress without fields ("Document is not owned by your company"), no standard content | the assembly and its parts once; no standard content or mirror items; linked parts left out (company ownership: none offline) | – | = (ownership: cloud) |
| Drawing pending update | red band "Drawing has a pending update" blocks; after Update it releases, title block DRW-1 / A, tab menu Revision history… | ? | – | ? |
| Locking | released version: Name, Category, Part number, Revision, State, Material locked; workspace part keeps Part number editable | ? | – | ? |

## Help (rows live hasn't shown yet; until 2026-11-14 all came from here)

Key: `RD` crates/release-ui/src/release_dialog.rs, `RP` release_prefs.rs, `OPS` crates/ops/src/release.rs, `RC`
crates/ops/src/release_conditions.rs, `RS` crates/ops/src/release_settings.rs, `ML` crates/model-core/src/release.rs, `HP`
crates/history-ui/src/history_panel.rs, `WS` crates/app/src/workspace.rs. Help pages: TRW typical release workflow, OBS
obsoleting parts, RM release management, SETUP setting up release management, RMS Company settings › Release management,
RT revision table, CW customized release workflow; BUNDLE the client's strings. Line numbers are as of 2026-11-14.

## 1. Create Release candidate dialog

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Title | "Create Release candidate" (TRW, release-dialog2 / drawing-release-dialog-02.png) | RD:2182 | – | = |
| Refresh button left of title, creator only, while creating | "Refresh button to the left of the dialog title… only when creating" (TRW) | RD:2220 (Mode::Release only); tooltip "Refresh" (BUNDLE "Refresh Release candidate") | – | = (tooltip text ≠) |
| ⋯ overflow at top right | image drawing-release-dialog-02; "View audit log", "View workflow" (CW) | RD:1655-1710 | – | = |
| Header "Comments" button | not in the creator's or reviewer's dialog images (drawing-release-dialog-02, reviewreleasedialog) | RD:2226 (always shown) | – | see the live matrix |
| Dialog and sections resizable | "You can resize this dialog and its sections by clicking and dragging an edge or corner" (TRW) | the right edge / corner (re-centred), the bottom edge, the list / form splitter and the items / columns splitter (`RC_SIZE`, `RC_ITEM_W`) | – | = (live 11-14) |
| Yellow triangle on new/modified fields | "New or modified fields are marked with a yellow triangle in the upper left corner" (TRW) | RD:1794, RD:1821 | – | = |
| Undo / Redo buttons + OS hot keys | "use the Undo and Redo buttons or your operating system hot keys" (TRW); image: ↶ ↷ at bottom left | RD:2062-2063, keys RD:2202-2211 | – | = |
| Item row: thumbnail, type icon + name, italic type, red × | image drawing-release-dialog-02 | the same ("Remove from release") | – | = |
| Red × removes item; tooltip | "clicking the red x" (TRW); BUNDLE tooltip "Remove from release" | RD:1855 tooltip "Remove" | – | see the live matrix |
| Click item name opens it "in another browser tab" (dialog stays, then Refresh) | TRW "The appropriate tab will open in another browser tab… return… click Refresh" | RD name click: another window on the document at that tab (`release_hooks::open_tab_window`); the dialog stays | – | = (fixed 11-14) |
| Search by name | "search … by name" (TRW, filter-object-list.png) | RD:1586, RD:1763 | – | = |
| Filter icon expands type toggles + Clear | TRW "Click the Filter icon… type icons… Clear" | "Toggle filters" only with more than one item type; a toggle per type listed and Clear (greyed until one is on) | – | = (live 11-14) |
| Include associated drawings button | TRW "Optionally, click Include associated drawings at the top" | RD:1592-1604; OPS:202-221 | e2e release_leaves_out_a_drawing_made_from_another_workspace | = |
| Drawings from another workspace left out, with bubble | TRW; BUNDLE "Drawings were unable to be added from workspaces: " / "No associated drawings found." | RD:1595-1601 | e2e release_leaves_out_a_drawing_made_from_another_workspace | = |
| Outdated associated drawing: warning, can update or include | TRW "Warnings are shown for any outdated associated drawings. You can update them or include" | a red band "Drawing has a pending update" (yellow with the condition off); no action in the dialog, as live | – | = (live 11-14) |
| + Add to Release… | TRW "Use the plus sign to add more objects" | RD:1605-1608 | – | = |
| Columns Revision / State / Part number | image | RD:1618-1641 | – | = |
| Revision filled from scheme and earlier releases | TRW "Revision is automatically supplied … incremented automatically" | ML:396-403, OPS:78 | e2e release_parts_and_revision_history; ML test releases_obsolete_and_validate | = |
| Revision can't go backward | TRW "if the previous revision is B … you may not change to A" | ML:438-448 (RevisionTooLow) | e2e release_parts_and_revision_history | = |
| Imported revision kept on first release; mismatched scheme → warning and adjusted (e.g. 6 → A) | TRW (revision-update.png) | a valid typed revision kept; one the scheme lacks starts at the next with "6 is not a valid revision … is A." (yellow) | – | = (live 11-14) |
| Revision / part number error texts | BUNDLE "Revision must not be blank.", "Part number must be unique." | ML:344-347 "Revision does not follow the revision scheme.", "Part number is used by another item of this release." | ML test releases_obsolete_and_validate | see the live matrix |
| State column: In progress (workspace), Released (linked released) | TRW "In progress… Pending… Released reflects previously-released, linked objects" | RD:1769-1774; linked released → reverted/Released (crates/release-ui/src/release_add_source.rs:160-176) | – | = |
| State Pending for items linked to a pending release | TRW "If a linked object is linked to a release that is Pending … Pending state" | add_source only checks released packages (release_add_source.rs:163); shows In progress | – | ≠ |
| Part number field per row, # "Generate next part number" (sequential on) | TRW; image (greyed when filled) | RD:1873-1892 | ML test sequential_numbers_share_the_counter | = |
| Header # generates part numbers for the candidate | TRW "Clicking # at the top … generates part numbers for all of the objects" | RD:1632-1641 "Generate missing part numbers" (BUNDLE tooltip) | e2e release_drafts_revert_and_generate (generate_part_numbers) | = |
| Drawing shares part number with its single shown part (reuse on) | TRW "If either the Drawing or object… has an associated number… assigned to both… only if one object" | RD:1019-1055 | ML test drawings_may_reuse_a_part_number | = |
| Edit properties icon per row opens a Properties *view inside the dialog* (Apply, Go back / back arrow, pick other items) | TRW "Editing properties of objects" (properties-view.png) | a Properties view inside the dialog: ← Back to Release properties, the fields without Part number / Revision / State, Apply / Go back | – | = (live 11-14) |
| Export icon per row | TRW "Click the Export icon next to the Properties icon" | RD:1924-1930 | – | = |
| Revert button in State column; header Revert all unchanged | TRW "Reverting items…"; revert-01/-04.png; header tooltip "Revert unchanged items to last revision" | RD:1866-1872, RD:1619-1631; row tooltip "Revert to last revision" (BUNDLE "Revert item to last revision") | e2e release_drafts_revert_and_generate | = (row tooltip text ≠) |
| Blue messages: unchanged / properties changed (± company policy) | revert-03/-04/-04-01.png texts | RD:1939-1949 (exact texts) | – | = |
| "N items need attention" + filter toggle; removed when all fixed | TRW "Filtering items requiring attention" (error-01.png) | RD:1646-1654 | – | = |
| Release name: required (*), shown as version name | TRW "required … displayed as the version name" | RD:1985, OPS:367 | e2e release_parts_and_revision_history | = |
| Release name # "Generate next release name" (sequential release names) | TRW | RD:1992-2005 | – | = |
| Release notes; required by setting | TRW; RMS "Require a note" | RD:2007-2008, RD:1967 | – | = |
| Approvers field: label * when required, placeholder "Search users or teams" | images drawing-release-dialog-02 / reviewreleasedialog; BUNDLE | RD `people_field`: "Approvers *" when required, italic "Search users or teams" | – | = (fixed 11-14) |
| Observers field | TRW "View only rights … receive notifications" | RD:2017 (local user only) | e2e release_submit_approve_and_reject | cloud (field kept) |
| Approver chips with user/team, team = any one member, edit-permission check, "Share with mentioned approvers" | TRW, BUNDLE | – | – | cloud |
| No approvers → Release button releases at once | TRW "if this field remains empty then the objects are immediately released upon clicking Release" | RD:2113-2118, OPS:253 | e2e release_submit_approve_and_reject (R3) | = |
| Approvers listed → Submit: version made, candidate Pending | TRW "Submit (visible when approvers are listed) to create a version, and set … Pending" | RD:2119-2125, OPS:261-267 | e2e release_submit_approve_and_reject | = |
| Footer buttons and colours: Apply (blue), Submit (blue), Close (grey) | images drawing-release-dialog-02, reviewreleasedialog | RD footer: Apply, Submit / Release #1651b0, Close grey | – | = (fixed 11-14) |
| Save draft (bottom left) saves and closes | TRW "Save draft … saves … closes the dialog, and adds the new draft to the Release drafts panel" | RD:2064-2068 | e2e release_drafts_revert_and_generate | = |
| Bubble after Save draft | BUNDLE "<b>name</b> release candidate is saved in your release drafts." | none (save_draft RD:1282-1300 emits nothing) | – | see the live matrix |
| Close closes without recording | TRW | RD:2172 | – | = |
| Status text when blocked | BUNDLE "A field required by one or more actions is empty" | RD:2021 | – | = |
| Submit/Release disabled while items need attention | refresh-03 note "Submit is disabled" | RD:2020 (blocked) | e2e release_parts_and_revision_history ("needs attention") | = |
| Release conditions block with item messages | RMS "Release conditions"; refresh-03 text "Part belongs to a Part Studio with a Feature list that is in a rolled back state." | red bands with live's texts (rolled back, pending reference updates, drawing update) | RC test a_reference_to_an_obsoleted_release_is_obsolete | = (live 11-14) |
| Non-blocking notices when a condition is off (pending task, drawing pending update, references obsolete revision: obsolete-reference-01 warning) | RMS "If this condition is not checked … a message is still displayed"; TRW "Drawing has a pending update"; OBS "the following warning is displayed" | yellow bands, not blocking (`Issue::Notice`) | – | = (live 11-14) |
| Admin override when company requires approvers ("…as an administrator you can override this setting and __transition__…") | BUNDLE | none: with Require approver + Do not allow creator as approver the local user can never release (RD:1414, 2019-2020) | – | see the live matrix |
| Release from an Assembly lists its whole BOM incl. standard content (no revision) and linked parts (no new revision) | TRW "Releasing assemblies" | OPS:158-165 skips `External`, `Standard`, `MirroredPart` instances | – | see the live matrix |
| Release from a Drawing lists the parts/assemblies it shows | TRW "Releasing drawings" | OPS:171-184 | – | = |
| Workflow selection dialog (Select release workflow, Debug mode) when several workflows | CW (select-workflow-dialog.png, fs-custwkflow-1.png) | – | – | missing (Enterprise; only one workflow locally) |

## 2. Add items

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Title "Add Items to Release" | BUNDLE | RD:587 | – | = |
| Types: Part Studios, Assemblies, Files, Drawings, Variable Studios | TRW (add-item-to-release-01.png) | RD:143, RD:612 | – | = |
| Search per type | image | RD:157, RD:629 | – | = |
| Part Studio row expands to its parts; picks shown at right with red × | image | RD:477-562 | – | = |
| Add / Cancel with tooltips | BUNDLE "Add selected items to Release" / "Cancel adding items and resume editing Release" | RD:654-660 | – | = |
| Current document / Other documents (only with "Allow adding items from other documents") | RMS | RD:615-619 | – | = |
| Linked item shows version icon | (help Release__selecting_parts…, not saved locally) | RD:1837-1845 | – | ? (page not in local set) |

## 3. Review / approve / reject / drafts

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Release drafts panel (left icon), double-click opens draft | TRW "click the Release drafts icon on the left side… double-click on a draft" | WS:2175, RD:2355-2427 | – | = |
| Draft reopens titled "Review Release" | image reviewreleasedialog ("Review Release") | RD title: "Review Release" | – | = (fixed 11-14) |
| Save draft ▴ menu: save / delete | TRW; BUNDLE "Save draft release" / "Delete draft release" | RD:2069-2103 "Save draft" / "Delete draft" | e2e release_drafts_revert_and_generate | see the live matrix |
| Delete draft confirmation | BUNDLE "Delete release candidate draft?" / "This will permanently delete <b>name</b> and cannot be undone." | RD:2254-2281 | – | = |
| Draft submitted from Review Release | TRW "click the Submit button to submit your Release" | RD:2108-2125 (same footer) | e2e release_drafts_revert_and_generate (from_draft) | = |
| Approve → Released at the Submit version; "Approved by …" | TRW, BUNDLE | RD:2145-2149, OPS:270-276, RD:2177 | e2e release_submit_approve_and_reject | = |
| Reject → Rejected; "Rejected by …" | TRW, BUNDLE | RD:2140-2144, ML:490-499 | e2e release_submit_approve_and_reject | = |
| Discard Release candidate (creator) | BUNDLE strings | RD:2238-2250, OPS:279 | e2e release_submit_approve_and_reject | = |
| Comments at any time | OBS "Comments or notes can be entered at any time" | RD:1337-1391, OPS:288 | e2e release_submit_approve_and_reject | = (local; no notifications) |
| Notifications, Action items page, blue "submitted" bubble | TRW | – | – | cloud |
| Require approval from all approvers / creator override of other approvers | RMS, BUNDLE | – | – | cloud |
| Delegating approval (My account › Preferences) | SETUP | – | – | cloud |
| Released package review: read-only + Clone | (Release__cloning_a_release_package, not saved; BUNDLE "Recreate Release from the current workspace") | RD:2159-2169 | – | see the live matrix |

## 4. Revision history

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Opened from part / Assembly / Variable Studio / Drawing context menu | OBS "part in a Parts list, Assembly, Variable Studio, or Drawing tab" | part list crates/part-studio-ui/src/part_studio/mod.rs:4783-4785; tab WS:1985-1987 (only once it has revisions) | – | = |
| Title "Revision history", sub-header part number (bold, ~16 px) | release_revision-history-01.png ("A-101") | RD:2530: number at 12 px, muted | – | see the live matrix |
| Revisions newest first | image (C, B) | RD:2438 | e2e release_parts_and_revision_history | = |
| Icons: ▲ released (black); obsolete icon; auto-obsoletion icon | OBS; image | RD:2453: ▲ green / △ grey for any obsolete; text "Obsoleted by a later revision" / "Obsolete" | – | see the live matrix |
| Row text order: user name, release name (bold), date, Version: name ↗, Document:, Type: | image | RD:2460-2485: release name + date, "User: …", Document, Version, Type | – | see the live matrix |
| Buttons stacked: View release, Export, Obsolete revision (red) | image | RD:2489-2526 in a row: Export, View release, Obsolete revision | – | see the live matrix |
| View release opens Review Release | OBS | RD:2504-2507 | – | = |
| Export opens Export dialog for the release | OBS | RD:2491-2502 | – | = |
| Click version name → opens version "in a new tab" | OBS | RD:2464-2482: opens the version in the same window | – | see the live matrix |
| Obsoleted revision: button becomes "View obsoletion" | OBS; image (B) | RD:2510-2526 for obsoletion packages; an auto-obsoleted revision keeps "Obsolete revision" and has no View obsoletion | – | see the live matrix |
| Revision label reuses skipped labels? no ("not reusing any skipped labels") | OBS | ML:396-403 | ML test revision_scheme_skips_letters | = |
| Op | – | `release_revision_history` (crates/ops/src/ops/release.rs:103) | e2e release_parts_and_revision_history | = |

## 5. Obsoletion

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Obsolete revision opens "Create Obsolete candidate" | OBS | RD:2519-2525, RD:2184 | – | = |
| "Mark revision as re-releasable" checkbox; label reused next release | OBS | RD:2009-2015, ML:399 | ML test releases_obsolete_and_validate | = |
| Admin can make an already obsoleted revision re-releasable | OBS "an administrator can override the option" | View obsoletion › Mark revision as re-releasable, Apply (op release_re_releasable) | ML test reset_part_number_type_obsoletes_that_type_only | = (live 11-14) |
| Add to Obsolete candidate (+) → "Add Items to Obsoletion" | OBS; BUNDLE title "Add Items to Obsoletion" | RD:1611-1616, RD:587, RD:360-390 | – | = |
| Submit when an approver is given, else Obsolete | OBS "Click Submit if an approver's email is supplied, otherwise click Obsolete" | no Approvers/Observers in obsolete mode (RD:2017); only Obsolete (RD:2151-2158) | – | see the live matrix |
| Name / notes fields and which are required | OBS "Supply the required information" | label = display name "Obsoletion names"/"Obsoletion notes" (RD:1964, RS:79); notes required (RD:1967, 1489) | – | see the live matrix |
| Obsoleting marks the released version (solid ▲ with dot) | RM | OPS:364-368, HP:1110-1162 | – | = |
| Obsoleted revisions never offered when inserting into Assembly / Drawing | OBS | drawing insert filters them (crates/drawing-ui/src/drawing_chooser.rs:117-127); assembly Insert has no release filter | drawing_chooser released_items_newest_first_without_obsolete | = (drawing) / ? (assembly) |
| Agent API op to obsolete a revision | AGENTS.md: every app action is an op | ops `release_obsolete` | e2e release_parts_and_revision_history | = (fixed 11-14) |

## 6. Company settings › Release management

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Header "Release management settings for objects in documents owned by …" | accounts_co-rm-02.png | RP:502 | – | = |
| Save release settings at top and bottom, disabled until a change | RMS; image (pale) | RP:253-254, RP:503, RP:512 | – | = |
| Enable managed workflows; off = tools off | live2 2026-11-14 (Professional): off, a part's menu has no Release… (no upsell), Versions rows no Releases, no Create release candidate; Properties has no release icon and State is a plain dropdown | `managed_workflows()` gates the part / tab / instance menus, Releases, the panel button, the Properties icon; State a select when off | RS test old_settings_files_keep_their_values_and_get_the_defaults | = (live 11-14; ours starts on, live's new company starts off: asked) |
| Release workflow / Obsoletion workflow tabs with diagrams + description | relmgmt-workflows-13/-04.png | RP:271-308, RP:600-686 | RP test the_workflow_charts_parse | = |
| "Onshape default release workflow ↗" View in document link | RMS "external link icon … opens the workflow document" | a public Onshape document: cloud | – | cloud |
| Enterprise custom workflows (Active, Published date/by, Unpublish, preview) | RMS, CW | – | – | missing (Enterprise) |
| Revision scheme select: "Alphabetical A B C skipping I O Q S X Z", Numerical, "New custom revision scheme…" | RMS; relmgmt-workflows-14.png | RP:22-25, ML:70: Alphabetical, "Numeric 1 2 3", "Numeric 01 02 03", "Numeric 001 002 003", customs, New custom… | ML test numeric_and_custom_revision_schemes | ? (help names one "Numerical"; three numeric variants not in help) |
| New custom scheme form: Name * , Revision list, preview, Download sample, Select file, Save revision scheme (disabled), ≥50 lines | Capture-01.png; RMS | RP:314-369, RS:19 | – | = |
| Revision scheme id + Copy to clipboard | image (Alphabetical id 59f7562a39d71809e38162e5) | RP:388-396; id is our FNV hash (RS:180-189) | RS test scheme_ids_are_stable_and_distinct | ≠ (id value) |
| Unreleased revision suffix, default "*" | RMS | RP:397-398, RS:130 | – | = |
| Revision table template row + "Click here to go to release management part number generation settings" | image | RP:399-410 | – | = |
| Access: 3 checkboxes, first checked by default | relmgmt-workflows-15.png; TRW "By default … Only administrators can edit … is checked" | RS `default_access`: the first checked | ops old_settings_files_keep_their_values_and_get_the_defaults | = (fixed 11-14) |
| Actions: Require a new revision when an item's properties have changed | RMS (BUNDLE label "have changed") | RP:420-426; used RD:1757 | – | = |
| Watermarks: Show watermark for drawings / Hide … not revision managed / Edit watermarks | watermark-options-01.png | RP:429-446 | – | = |
| Watermarks dialog: 6 states, defaults (Released empty), 64 chars, no Unicode, Save changes / Cancel | watermarks-dialog.png; RMS | RP:160-226, RS:34-36 | RP test the_workflow_charts_parse (64/ASCII) | = |
| Watermark on drawing: In progress → removed when released → "Obsolete" | RMS; example-drawing-watermark.png | RD:77-96 | drawing_watermark the_watermark_runs_along_the_diagonal | = |
| Watermark for Pending / Rejected / Discarded states | watermarks-dialog.png (texts per state) | the Watermarks dialog in live's order and defaults; Edit watermarks greyed until watermarks are on | – | = (live 11-14) |
| Release conditions: 8 checkboxes in order | RMS | RS:56-65, RP:449-456 | – | = |
| Create Release candidate dialog: 6 checkboxes, "Do not allow creator as approver" disabled until Require approver | relmgmt-workflows-18-01.png | RP:468-489 | – | = |
| Do not allow creator as approver: creator's name not in the list | RMS "the creator's name doesn't appear in the list" | RD:1414, 1444-1445: shown greyed with a tooltip | – | see the live matrix |
| Restrict approvers to specific users or teams | RMS (Professional) | – | – | cloud |
| Display names for Release name / notes, Obsoletion names / notes | RMS | RP:460-467, RS:166-173 | RS test old_settings_files_keep_their_values_and_get_the_defaults | = |
| Obsoletion: Automatically obsolete previous released revisions (default off) | relmgmt-workflows-19.png; RMS | RP:492-498, ML:371-375 | ML test releases_obsolete_and_validate | = |
| Agent API ops for these settings | AGENTS.md | ops `company_release_settings` | e2e company_release_settings_and_numbering_schemes | = (fixed 11-14) |

## 7. Numbering schemes (help page not saved; rows from TRW / RMS only)

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Sequential part number generation enables the # icons | TRW | crates/release-ui/src/numbering_prefs.rs:21 (Manual entry / Sequential…); RD:988 seq_parts | ML test sequential_numbers_share_the_counter | = |
| "Drawing can reuse part number from an assembly or part in the release" | TRW | numbering_prefs.rs:291-299; ML:431 | ML test drawings_may_reuse_a_part_number | = |
| Release names: sequential number generation → # on Release name | TRW | numbering_prefs.rs:18 (tab "Release names"); RD:1992 | – | = |
| Link from Release management to the numbering settings | RMS | RP:401-409 | – | = |
| Tabs, prefixes, Min length, Next, propagation, Save changes per tab | (Plans/numbering_schemes not saved) | 4 tabs, each its own Save changes; Release objects' prefixes PRT- ASM- DRW- VS- FILE-, min length 3, next 1 | – | = (live 11-14) |
| Agent API ops for numbering schemes | AGENTS.md | ops `company_numbering_schemes` | e2e company_release_settings_and_numbering_schemes | = (fixed 11-14) |

## 8. Revision table template / revision table

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Default "Onshape Default revision table": Revision, Revision description, Date approved, Approver | relmgmt-workflows-14.png | crates/drawing/src/drawing/tables.rs:184-188 | tables revision_table_rows | = |
| Edit → rename, add/remove columns, reorder, all uppercase | RMS | crates/release-ui/src/revision_template.rs:136-296 (drag reorder, Revision not removable) | – | = |
| Save button text | RMS "To save your changes, click Create"; BUNDLE "Save changes" | revision_template.rs:296 "Save changes" | – | = (bundle over help) |
| Pending revision row highlighted blue until released; suffix "*" | RT | tables.rs:155-165, 233 | tables revision_table_rows; drawing tests formats_gaps_and_revision_tables_across_sheets | = |
| Agent API op for the template | AGENTS.md | ops `company_release_settings` (`revision_table`) | e2e company_release_settings_and_numbering_schemes | = (fixed 11-14) |

## 9. Entry points, version graph, history panel

| Item | Help says | Ours | Test | Status |
|---|---|---|---|---|
| Parts list › right-click › Release… (one or several parts) | TRW | crates/part-studio-ui/src/part_studio/mod.rs:4774-4782, 7936 | ui-parity part-menu-release; list_feel_tests rows | = |
| Tab › Release… for Assembly, Drawing, Variable Studio, file | TRW | WS:1977-1984, WS:2239 | ui-parity vs-tab-release | = |
| Tab › Release… for a Part Studio | TRW lists Part Studio | none (live: no Release on a Part Studio tab, behaviours 13310) | – | = (live over help) |
| Assembly top row / instance › Release… | TRW; live 2464 | crates/assembly-ui/src/assembly_behaviors.rs:1470, 2282, 2606, 2736 | ui-parity asm-menu-release; assembly_ui_tests the_instance_and_top_row_menus_are_lives | = |
| Properties dialog release icon (top left) | TRW "From any Properties dialog … part, Part Studio, Assembly, Drawing, Variable Studio, or File" | parts, assemblies, drawings, Variable Studios, files and now Part Studio tabs | – | = (live 11-14) |
| Versions and history header › Create release candidate (active tab's objects) | TRW | HP:1617-1624 | ui-parity ver-vh-header-release | = |
| Version graph: released version solid ▲ | RM | HP:1150-1158 (lane colour) | – | = |
| Version graph: Pending / Rejected candidate open △ | RM "marked with an open triangle" | HP release marks: open △ (`ReleaseLog::candidate_at_version`) | model-core pending_and_rejected_candidates_mark_their_version | = (fixed 11-14) |
| Version graph: obsoletion ▲ with dot | RM | HP:1159-1165 | – | = |
| Legend: Release candidate, Release, Contains obsoletion | RM; live legend | HP:1756-1757 | – | = |
| Version row › Releases ▸ submenu | live ver-vrow-releases | HP:1436-1448 ("No releases" / "<name> [Released]") | ui-parity ver-vrow-releases | = (item format ?) |
| Not revision managed (Properties checkbox) → no revision | TRW "Marking objects as not revision-managed" | OPS:75-78, RD:65-69 | – | = |
