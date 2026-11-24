# Part Studio feel-coverage audit (psui, 11-24)

The Part Studio specs in live's behaviours.md (feature list, feature dialogs, viewport hover / select, manipulators,
context menus, view cube / navigation, measure / mass properties), matched against our tests: first by a test or an
interaction-feel row citing the spec, then by hand for the rest (a test of the same behaviour that doesn't cite it).
Scripts and per-section verdicts: psui's scratch `audit/` (scan.py, triage.tsv).

| | sections |
|---|---|
| Part Studio specs found | 186 |
| out of scope (drawings, assemblies, sheet metal, other locales, dark theme, model geometry) | 22 |
| in scope | 164 |
| covered before, cited by a test | 72 |
| covered before, by a test that doesn't cite it | 70 |
| **covered before** | **142** |
| added in this audit | 11 |
| **covered after** | **153** |
| still without a test | 11 |

New tests: `part-studio-ui/src/part_studio/ps_audit_feel_tests.rs` (`audit::`), one row in the navigation matrix and
one in the feature-list matrix. The audit found no behaviour of ours off live: every new test passed once its fixture
was right (the app only gained debug selectors for the tests: the list title, the dialog ✕, the view cube, a Loft entry's ›).

## Added in this audit

| behaviours.md line | spec | test / note |
|---|---|---|
| 4966 | Dialog ✓ / ✕ tooltips (psb 10-21 e) | audit::a_dialogs_ok_and_cancel_have_no_tooltip |
| 13116 | qa4 round 4 s5 multi-part menu | list row "menu / every part: no Hide other / all parts" (+ "menu / two parts") |
| 17395 | psui 2026-10-26: feature dialogs highlight only the active field's picks | audit::a_mirrors_part_is_lit_only_while_its_field_is_active |
| 17962 | psui (10-26) Loft Profiles / Guides entries: the › expander; Fillet Variable rows | audit::a_loft_profiles_expander_opens_its_field_as_the_active_one |
| 18416 | psui (10-27, PANEL-BLEED) Shift+D / Shift+C while a feature dialog is open | audit::an_analysis_key_over_a_feature_dialog_keeps_it_and_esc_closes_the_panel_first |
| 19310 | spec: psui part-studio.md 6.10 / 6.11 feature list filter box (name, type, commands) | audit::the_feature_filter_counts_in_the_title_and_hides_the_error_icon |
| 19412 | spec: psui part-studio.md 8.8 / 8.10 graphics context menu: modes (isolate, transparent, section, tool) and th | audit::the_empty_space_menu_leads_with_the_active_mode |
| 20867 | spec: navigation — view cube ◀ ▶ ▲ ▼ triangles = the arrow keys (15° per click) (parity workflow-keyboard-mous | nav row "view cube / a triangle click turns as its arrow key" |
| 23828 | qa (2026-10-28, P2) feature dialog ✓ state with a required field still empty (Revolve without axis, Mirror wit | audit::a_fillets_ok_is_off_until_a_pick_and_stays_on_after_the_chip_goes |
| 23893 | qa (2026-10-28, P3) ✓ after the last selection is removed: stays enabled (request 1575) | audit::a_fillets_ok_is_off_until_a_pick_and_stays_on_after_the_chip_goes |
| 33225 | psui (2026-11-03 15:05, P2) preselected edge into Fillet: a click on the edge's midpoint doesn't carry into Fi | audit::a_preselected_edge_carries_into_fillet_its_midpoint_does_not |

## Still without a test (next batches)

| behaviours.md line | spec | test / note |
|---|---|---|
| 5218 | Panels vs feature dialogs at the top-left (ui 10-21, lead) | panel positions vs dialogs: no position test |
| 5258 | Input behaviour outside feature dialogs (ui 10-21, lead) | analysis panel number fields (Zebra stripe count, Draft minimum angle): no test |
| 6364 | Hole dialog: Counterbore / Countersink icon rows (qa5 ADV4, 10-21) | Hole icon rows look |
| 18585 | psui (10-27, PANEL-BLEED follow-up) default dialog position: the analysis panel sits on top of the feature dia | analysis panel position over a dialog (x 246, covers 14 px) |
| 18606 | spec: psui DCLICK-HEAVY double-click on a feature row of a heavy part (live 10-27; TICKETS P1 DCLICK-HEAVY) | DCLICK-HEAVY latency on a heavy part |
| 19497 | spec: psui p1-dialogs.md long checkbox labels: live text widths and truncation | checkbox label widths: look spec |
| 19949 | psui (10-27) Delete part preview: outline and ghost at slider rest, 0 and 1 | Delete part preview look (silhouette only) |
| 21717 | psui (2026-10-27, P3) MENU-TRUNCATE: long feature names in context menus (request 1509) | context menu 250 px max width / ellipsis: implemented (os_menu max_w 250), no test |
| 21819 | qa (2026-10-27, P2) PREVIEW-SELF-PICK: hovering / clicking a feature's own preview geometry (request 1510) | preview self-pick (hover resolves pre-feature) |
| 23319 | qa (2026-10-28) Part Studio empty-space right-click › Show all / Show all parts (request 1562) | Show all / Show all parts effect (menu order tested) |
| 37222 | psui (2212): Camera menu's disabled rows, the start tab rule, and Ruled surface's cubic interpolation default | Camera menu disabled rows: no test |
