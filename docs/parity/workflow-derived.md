# Workflow: derived parts and references between Part Studios (Derived feature, source changes, configurations, in-context)

qa5, 2026-10-21/22 pass 9, app /tmp/qa5/noshape-2330 (target-shared 23:30, tree = main 5bd50b2 + uncommitted work).
Docs: /tmp/qa5/d23/derive.noshape (setup /root/qa5-scratch/pass9/mkderive.py: "qa5 src" = 40×20×10 block, empty "qa5 dst", Assembly 1)
and /tmp/qa5/d23/cfg.noshape (copy of the pass 7 "qa5 config" doc: Part Studio 1 with inputs d = 10 mm and List 2 Default/Large + a new "qa5 cfg dst").
Shots /tmp/qa5/shots/d*.png, c*.png; raw notes /root/qa5-scratch/pass9.md.
Live: behaviours.md "Derived from a configured Part Studio …" (psb 10-20 j/l, part-studio/400–447), "In-context editing, persisted" (skb, assembly/500–524).
Live request "qa5 (10-21 pass 9, workflow 23 "derived parts" …)" filed for the "?" rows.
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Alt+C "derived" | Derived (toolbar list has no "Derived mirror") | Derived + **Derived mirror** listed | ? (asked) |
| 2 | Empty Derived dialog | "Derived 1" red; Part Studio field **pink/red error box** "Select Part Studio…"; ☐ "Preserve active sheet meta…" (label 174 px); full-width "Locations"; **"Placement" label** + [Base origin ▾]; ☑ Include mate connectors; ☑ Include properties; slider, (?) | no pink error state (grey outline); **no "Placement" label** (select full width); **extra clock icon right of Locations** (field narrower); label clips later ("…metal m…"); rows ~4 px tighter (d03, cmp-d03) | ✗ **DER1** (psb) |
| 3 | Select Part Studio panel | 300 px panel, tabs, doc header + 2 icon buttons (insert-from-version, **branch**), search, 64 px thumbnails | same layout; second icon is a list/tree glyph, ✕ smaller and lighter (d04) | ✗ DER2 (psb, minor) |
| 4 | Pick the studio | row turns blue **and expands inline**: ‹ back arrow in the header, contents (Part 1 row highlighted, Sketch 1), part chip next to [Generate] (407) | row turns blue but stays **collapsed** (›); after a click on › the inputs/Generate/contents show, but no ‹, no part chip, part row not highlighted (d05, c02, c03); reopening the field (live 418: expanded) is collapsed too (c09) | ✗ **DER3** (psb) |
| 5 | ✓ | Derived 1, Parts (1) Part 1 | same (d06) | ✅ |
| 6 | Source Extrude 1 depth 10 → 15 | derived part follows (same workspace) | follows at once (d08) | ✅ (? live confirm asked) |
| 7 | Rename the source part "Block" | ? | derived part keeps "Part 1" (d10) | ? (asked) |
| 8 | Fillet 2 mm on a derived edge, then reopen Fillet 1 | chip "Edge of Derived 1" (normal entity naming) | fillet regenerates fine, but the chip reads **"Edge of deleted feature" in red** (d13) | ✗ **DER4** (psb) |
| 9 | Source gets Part 2 (cylinder) | whole-studio derive picks it up | Parts (2), fillet survives (d18) | ✅ (? asked) |
| 10 | Suppress source Extrude 1 | ? | Derived 1 stays OK (only the cylinder, renamed "Part 1" as in the source); Fillet 1 red "Fillet 1 did not regenerate properly: Selected edges no longer exist" (d20, d21); undo restores | ? (asked) |
| 11 | Derived 1 row right-click | ? | Rename, Edit…, Add selection to folder…, Suppress, Dynamic suppression ▸, Zoom to selection, Show dependencies…, Roll to here, Delete (d23) | ? (asked) |
| 12 | Feature-list Derived row | rows carry a **▸ expander** (421) | no expander | ✗ **DER5** (psb; content asked) |
| 13 | Derive the same studio + configuration again | dialog title black, **✓ enabled**; accepted, then row/title red with tooltip "Derived 2 did not regenerate properly: Creating multiple Derived features from the same source and configuration is not allowed. Consider editing the previous Derived feature." (421–425) | title red and **✓ disabled before accepting** (can't be accepted); the title tooltip text is identical (d24–d26, c08) | ✗ **DER6** (psb) |
| 14 | While that second Derived dialog is open | other parts drawn normally | Derived 1's **cylinder turns into a faint ghost** (block stays solid) (d24, d53) | ✗ DER7 (psb) |
| 15 | Placement ▾ options | ? | Base origin, Base mate connector (d27) | ? (asked) |
| 16 | Assembly › Insert › Part Studios list | a Part Studio with derived parts lists them like any parts | **"qa5 dst" expands to nothing (no thumbnail, no rows) and clicking it inserts nothing**; same in the configured doc (d28–d30, c11, c15) → **derived parts can't be inserted into an assembly** | ✗ **DER8** (skb, severe) |
| 17 | Part right-click › Copy Block, Ctrl+V in another Part Studio | Derived dialog with the source studio and part preselected | "Derived 2" opens with "qa5 src" filled (d52). Ctrl+C after a plain parts-list click copied nothing (d50) | ✅ **DOC3 fixed**; Ctrl+C ? |
| 18 | Panel contents after the source part was renamed "Block" | the part's current name | lists **"Part 1"** (source and Insert dialog say "Block") (d53) | ✗ **DER9** (psb) |
| 19 | Pick only Part 2 in the expanded panel | ? | replaces the selection; field stays "qa5 src" (d54) | ? (asked) |
| 20 | Configured source: change List 2 → Large | contents list **empties** and [Generate] enables; Generate lists them again, Generate disabled (409–410) | Generate enables, but the contents stay listed; Generate regenerates the preview (symmetric extrude) and disables (c05, c06) | ✗ DER10 (psb, minor) |
| 21 | Derived 2 with the Default configuration next to Derived 1 Large | accepted; Parts (2) "Part 1", "Part 1" (426) | same (c07) | ✅ |
| 22 | Reopen the configured Derived 1 | configuration kept (Large) | kept (c10) | ✅ |
| 23 | Close/reopen the doc, open "qa5 cfg dst" | Parts (1) | **Parts (0)** and no Appearance strip button until the tab is left and re-entered (c13, c14) | ✗ TAB1 (psb), also for derived studios |
| 24 | Assembly instance right-click (before a context) | live list (assembly/501) | same minus the cloud-only Add comment (d32) | ✅ |
| 25 | Edit in context | grey banner "<asm> displayed" [Go to assembly] [Hide/show instances] ✕, 2nd row checkboxes, slider; edited part selected | same (d33) | ✅ |
| 26 | Plane › pick a ghost edge/face | context created: amber banner "Context 1 of <asm>" [Hide/show instances ▾] [Done]; "Assembly contexts" + disabled dropdown "Context 1 <asm>"; chip "… of Context 1" (504) | same (d37, cmp-504) | ✅ |
| 27 | ✓ | Plane 1 row: yellow ⇦, tooltip "Context 1"; dropdown + ⋯ | same (d38) | ✅ |
| 28 | Banner ▾ | Go to assembly · Hide/show instances | same (d39) | ✅ |
| 29 | Back in the assembly | primary instance: solid ⇦ "Context 1" | same (d40, d40i vs live 517) | ✅ |
| 30 | Another part of the same studio in the assembly (Part 2 <1>) | ? (live's dashed "not primary instance" ⇦ was on a 2nd instance of the same part) | no icon (d40) | ? (asked) |
| 31 | Move Part 2 <1> → context out of date | one icon (short ⇦ + larger blue dot touching it), tooltip **"Update available for Context 1"** over the whole icon | blue dot appears ✅, but the arrow keeps its own tooltip "Context 1" and only the dot says "Update available for Context 1"; shape: long ⇦ + small dot with a gap (d41, d42, cmp-icon) | ✗ DER11 (skb, minor) |
| 32 | Right-click › Update context ▸ • Context 1 | silent update, plain arrow again | same (d44–d47) | ✅ |
| 33 | Reopen the source Part Studio | dropdown "None", Plane 1 black outline ⇦ | same (d48) | ✅ |
| 34 | Plane 1 selected: bottom readout | ? | **"Area: 4000000000000.000 mm²"** for a datum plane (d48) | ? DER12 (asked) |
