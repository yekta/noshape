# Workflow: derived parts and references between Part Studios (Derived feature, source changes, configurations, in-context)

qa5, 2026-10-21/22 pass 9, app /tmp/qa5/noshape-2330 (target-shared 23:30, tree = main 5bd50b2 + uncommitted work).
Docs: /tmp/qa5/d23/derive.noshape (setup /root/qa5-scratch/pass9/mkderive.py: "qa5 src" = 40×20×10 block, empty "qa5 dst", Assembly 1)
and /tmp/qa5/d23/cfg.noshape (copy of the pass 7 "qa5 config" doc: Part Studio 1 with inputs d = 10 mm and List 2 Default/Large + a new "qa5 cfg dst").
Shots /tmp/qa5/shots/d*.png, c*.png; raw notes /root/qa5-scratch/pass9.md.
Live: behaviours.md "Derived from a configured Part Studio …" (psb 10-20 j/l, part-studio/400–447), "In-context editing, persisted" (skb, assembly/500–524).
Live request "qa5 (10-21 pass 9, workflow 23 "derived parts" …)" filed for the "?" rows.
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Alt+C "derived" | Derived (toolbar list has no "Derived mirror") | Derived + **Derived mirror** listed | ✅ (Alt+C "derived" lists only Derived) | a6c56638 10-28 |
| 2 | Empty Derived dialog | "Derived 1" red; Part Studio field **pink/red error box** "Select Part Studio…"; ☐ "Preserve active sheet meta…" (label 174 px); full-width "Locations"; **"Placement" label** + [Base origin ▾]; ☑ Include mate connectors; ☑ Include properties; slider, (?) | no pink error state (grey outline); **no "Placement" label** (select full width); **extra clock icon right of Locations** (field narrower); label clips later ("…metal m…"); rows ~4 px tighter (d03, cmp-d03) | ✅ (DER1) | a6c56638 10-28 |
| 3 | Select Part Studio panel | 300 px panel, tabs, doc header + 2 icon buttons (insert-from-version, **branch**), search, 64 px thumbnails | same layout; second icon is a list/tree glyph, ✕ smaller and lighter (d04) | ✅ (DER2) | a6c56638 10-28 |
| 4 | Pick the studio | row turns blue **and expands inline**: ‹ back arrow in the header, contents (Part 1 row highlighted, Sketch 1), part chip next to [Generate] (407) | row turns blue but stays **collapsed** (›); after a click on › the inputs/Generate/contents show, but no ‹, no part chip, part row not highlighted (d05, c02, c03); reopening the field (live 418: expanded) is collapsed too (c09) | ✗ DERIVED-PICK-CHIP (expands inline with ‹ ✅; no "Part 1" chip next to Generate, part row not highlighted) | a6c56638 10-28 |
| 5 | ✓ | Derived 1, Parts (1) Part 1 | same (d06) | ✅ | a6c56638 10-28 |
| 6 | Source Extrude 1 depth 10 → 15 | derived part follows (same workspace) | follows at once (d08) | ✅ (? live confirm asked) | a6c56638 10-28 |
| 7 | Rename the source part "Block" | ? | derived part keeps "Part 1" (d10) | ? (asked) |  |
| 8 | Fillet 2 mm on a derived edge, then reopen Fillet 1 | chip "Edge of Derived 1" (normal entity naming) | fillet regenerates fine, but the chip reads **"Edge of deleted feature" in red** (d13) | ✅ (DER4: "Edge of Derived 1") | a6c56638 10-28 |
| 9 | Source gets Part 2 (cylinder) | whole-studio derive picks it up | Parts (2), fillet survives (d18) | ✅ (? asked) | v0.4.12 CLI 10-28 (bin/der9.py: Part Studio 2 Derived of the whole Part Studio 1 → Part 1 8000; PS1 gets a Ø12 × 15 cylinder → PS2 Parts (2) Part 1 8000, Part 2 1696.46; the fillet part not re-run) |
| 10 | Suppress source Extrude 1 | ? | Derived 1 stays OK (only the cylinder, renamed "Part 1" as in the source); Fillet 1 red "Fillet 1 did not regenerate properly: Selected edges no longer exist" (d20, d21); undo restores | ? (asked) |  |
| 11 | Derived 1 row right-click | ? | Rename, Edit…, Add selection to folder…, Suppress, Dynamic suppression ▸, Zoom to selection, Show dependencies…, Roll to here, Delete (d23) | ? (asked) |  |
| 12 | Feature-list Derived row | rows carry a **▸ expander** (421) | no expander | ✅ (DER5: ▸ lists Sketch 1, Sketch 2) | a6c56638 10-28 |
| 13 | Derive the same studio + configuration again | dialog title black, **✓ enabled**; accepted, then row/title red with tooltip "Derived 2 did not regenerate properly: Creating multiple Derived features from the same source and configuration is not allowed. Consider editing the previous Derived feature." (421–425) | title red and **✓ disabled before accepting** (can't be accepted); the title tooltip text is identical (d24–d26, c08) | ✅ (DER6: black title, ✓ enabled; red row with live's tooltip) | a6c56638 10-28 |
| 14 | While that second Derived dialog is open | other parts drawn normally | Derived 1's **cylinder turns into a faint ghost** (block stays solid) (d24, d53) | ✅ (DER7) | a6c56638 10-28 |
| 15 | Placement ▾ options | ? | Base origin, Base mate connector (d27) | ? (asked) |  |
| 16 | Assembly › Insert › Part Studios list | a Part Studio with derived parts lists them like any parts | **"qa5 dst" expands to nothing (no thumbnail, no rows) and clicking it inserts nothing**; same in the configured doc (d28–d30, c11, c15) → **derived parts can't be inserted into an assembly** | ✅ (DER8: Part Studio 2 lists Part 1, inserted as Part 1 <2>) | a6c56638 10-28 |
| 17 | Part right-click › Copy Block, Ctrl+V in another Part Studio | Derived dialog with the source studio and part preselected | "Derived 2" opens with "qa5 src" filled (d52). Ctrl+C after a plain parts-list click copied nothing (d50) | ✅ **DOC3 fixed**; Ctrl+C ? | a6c56638 10-28 (DOC3: Part Studio field filled) |
| 18 | Panel contents after the source part was renamed "Block" | the part's current name | lists **"Part 1"** (source and Insert dialog say "Block") (d53) | ✗ **DER9** (psb) |  |
| 19 | Pick only Part 2 in the expanded panel | ? | replaces the selection; field stays "qa5 src" (d54) | ? (asked) |  |
| 20 | Configured source: change List 2 → Large | contents list **empties** and [Generate] enables; Generate lists them again, Generate disabled (409–410) | Generate enables, but the contents stay listed; Generate regenerates the preview (symmetric extrude) and disables (c05, c06) | ✅ (DER10) | a6c56638 10-28 |
| 21 | Derived 2 with the Default configuration next to Derived 1 Large | accepted; Parts (2) "Part 1", "Part 1" (426) | same (c07) | ✅ |  |
| 22 | Reopen the configured Derived 1 | configuration kept (Large) | kept (c10) | ✅ | a6c56638 10-28 |
| 23 | Close/reopen the doc, open "qa5 cfg dst" | Parts (1) | **Parts (0)** and no Appearance strip button until the tab is left and re-entered (c13, c14) | ✅ (TAB1: Parts (1) after load, strip present) | a6c56638 10-28 |
| 24 | Assembly instance right-click (before a context) | live list (assembly/501) | same minus the cloud-only Add comment (d32) | ✅ | a6c56638 10-28 |
| 25 | Edit in context | grey banner "<asm> displayed" [Go to assembly] [Hide/show instances] ✕, 2nd row checkboxes, slider; edited part selected | same (d33) | ✅ | a6c56638 10-28 |
| 26 | Plane › pick a ghost edge/face | context created: amber banner "Context 1 of <asm>" [Hide/show instances ▾] [Done]; "Assembly contexts" + disabled dropdown "Context 1 <asm>"; chip "… of Context 1" (504) | same (d37, cmp-504) | ✅ | a6c56638 10-28 |
| 27 | ✓ | Plane 1 row: yellow ⇦, tooltip "Context 1"; dropdown + ⋯ | same (d38) | ✅ | a6c56638 10-28 |
| 28 | Banner ▾ | Go to assembly · Hide/show instances | same (d39) | ✅ | a6c56638 10-28 |
| 29 | Back in the assembly | primary instance: solid ⇦ "Context 1" | same (d40, d40i vs live 517) | ✅ | a6c56638 10-28 |
| 30 | Another part of the same studio in the assembly (Part 2 <1>) | ? (live's dashed "not primary instance" ⇦ was on a 2nd instance of the same part) | no icon (d40) | ? (asked) |  |
| 31 | Move Part 2 <1> → context out of date | one icon (short ⇦ + larger blue dot touching it), tooltip **"Update available for Context 1"** over the whole icon | blue dot appears ✅, but the arrow keeps its own tooltip "Context 1" and only the dot says "Update available for Context 1"; shape: long ⇦ + small dot with a gap (d41, d42, cmp-icon) | ✅ (DER11: ⇦• one icon, "Update available for Context 1") | a6c56638 10-28 |
| 32 | Right-click › Update context ▸ • Context 1 | silent update, plain arrow again | same (d44–d47) | ✅ | a6c56638 10-28 |
| 33 | Reopen the source Part Studio | dropdown "None", Plane 1 black outline ⇦ | same (d48) | ✅ | a6c56638 10-28 |
| 34 | Plane 1 selected: bottom readout | ? | **"Area: 4000000000000.000 mm²"** for a datum plane (d48) | ? (no readout for a selected datum plane now; live not captured) | a6c56638 10-28 |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
Fresh doc /tmp/qa5/d23b/derive.noshape (pass9/mkderive.py), configured /tmp/qa5/d23c/cfg.noshape, context /tmp/qa5/d23d/derive.noshape. Shots e01–e42. Live "qa5 derived" (behaviours.md ~7093) folded in.
| Row | Ours | Status | Verified |
|---|---|---|---|
| DER1 empty dialog | pink "Select Part Studio…" box, "Placement" label, no clock icon | ✅ **fixed** (e02) | a6c56638 10-28 |
| DER2 panel icons | branch icon | ✅ **fixed** (e03) | a6c56638 10-28 |
| DER3 pick the studio | expands inline (Part 1, Sketch 1), ‹ back arrow; reopening shows it expanded | ✅ (DERIVED-PICK-CHIP fixed) | a6c56638 10-28 (expands, ‹; but no part chip next to Generate / no row highlight: DERIVED-PICK-CHIP) · verified c8239e05 10-28 (TICKETS DERIVED-PICK-CHIP) |
| DER4 fillet chip on a derived edge | not red any more, but still **"Edge of deleted feature"** (live "Edge of Derived 1") | ✗ still (partly) (e09) |  |
| DER5 ▸ expander | present; expands to "Sketch 1" (= live 1582) | ✅ **fixed** (e06) | a6c56638 10-28 |
| DER6 second derive of the same source | ✓ enabled, accepted, red row "Derived 2 did not regenerate properly: Creating multiple Derived features from the same source and configuration is not allowed. …" | ✅ **fixed** (e10, e11) | a6c56638 10-28 |
| DER7 ghosting | none | ✅ **fixed** (e10) | a6c56638 10-28 |
| DER8 insert derived part into an assembly | "qa5 dst" lists "Block"; inserts "Block <1>" with its fillet | ✅ **fixed** (e18, e19) | a6c56638 10-28 |
| DER9 renamed source part | panel and dst Parts show "Block" | ✅ **fixed** (e17) | c725f68f 10-28 (Part 1 renamed "Block" in Part Studio 1 → Part Studio 2 Parts (1) "Block", the Derived panel lists Part Studio 1 › Block) |
| DER10 configuration change in the panel | contents empty + Generate enabled; Generate re-lists, disables | ✅ **fixed** (e33–e35) | a6c56638 10-28 |
| DER11 context icon | one icon (short ⇦ + dot), "Update available for Context 1" on the arrow | ✅ **fixed** (e42) | a6c56638 10-28 |
| TAB1 derived studio after reopen | Parts (1) + Appearance strip button at once | ✅ **fixed** (e31) | a6c56638 10-28 |
| Live (6) Derived row menu | Rename, Edit…, Add selection to folder…, Suppress, Dynamic suppression ▸, **Switch to qa5 src** ✅, Zoom to selection, Show dependencies…, Delete; **no "Add appearance to feature…"** (live has it; ours has it on Extrude rows) | ✗ **DER14** (psb, minor) (e05) |  |
| Live (4) suppress src Extrude 1 | Derived 1 black ✅, Parts (0) ✅, Fillet 1 red ✅ but tooltip **"…: Selected edges no longer exist"** (live "…: Select edges or faces to fillet.") | ✗ **DER13** (psb, text) (e21, e22) |  |
| (new) Parts list › right-click › **Rename** | **app crashes**: panic "hover style already set" (vendor/gpui-pre/src/elements/div.rs:843) from render_part_row (crates/app/src/part_studio/mod.rs:2831); reproduced twice; Shift+N inline rename works. Log /root/qa5-scratch/crash-0503-rename.log | ✗ **CRASH1** (ui, severe) (e13, e14) |  |

## Re-run pass 19 (2026-10-22 ~11:00–11:40, app /tmp/qa5/noshape-1013 = /root/target-lead/debug/noshape 10:13: the queue's 09:58 batch build = main 3ce6952 + the ready branches of that batch; CLI 08:11)
| Row | Ours | Status | Verified |
|---|---|---|---|
| CRASH1 Parts list › right-click › Rename | inline editor, no crash (z01) | ✅ **fixed** | 8bb8a6d1 10-28 (GUI: Parts list › Part 1 › Rename → inline editor with the name selected, 'Base' + Enter renamed, app alive) |
| DER4 fillet chip on a derived edge | "Edge of Derived 1" (z03) | ✅ **fixed** | a6c56638 10-28 |
| DER13 fillet error after suppressing the source | "Fillet 1 did not regenerate properly: Select edges or faces to fillet." (z04) | ✅ **fixed** | c725f68f 10-28 (wf-block Part Studio 2: Derived 1 of Part Studio 1, Fillet 1 R1 on its edge, Suppress Derived 1 → "Fillet 1 did not regenerate properly: Select edges or faces to fillet.") |
| DER14 Derived row menu | has "Add appearance to feature…" (z02) | ✅ **fixed** | c725f68f 10-28 (Derived 1 row menu: Rename, Edit… · Add selection to folder… · Suppress, Dynamic suppression · Switch to Part Studio 1 · Zoom to selection · Show dependencies… · Add appearance to feature… · Delete) |

## Re-check on main a0e360a3 (qa4, 2026-10-23 19:30–20:40, FRESH=1, /root/qa4-scratch/bin/noshape-main)
| Finding | Result |
|---|---|
| DER2 panel icons | ✅ second icon is the branch glyph |
| Row 4 (DER3) pick the studio | ✅ row blue, expands inline, ‹ back arrow, contents listed |
| DER5 row expander | ✅ "›" on the Derived row |
| DER7 second Derived of the same studio | ✅ parts drawn normally while its dialog is open, ✓ enabled |
| DER1, DER4, DER6, DER8–DER11, DER13, DER14, TAB1, CRASH1 | fixes cited in code; not re-run this pass |
