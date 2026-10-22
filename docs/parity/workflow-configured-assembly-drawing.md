# Workflow: drawing of a configured assembly with a BOM table, after a configuration change

qa5, 2026-10-21 pass 8, app /tmp/qa5/noshape-1924 (target-shared 19:24), doc qa5-cfgasm.noshape (see workflow-configured-assembly.md:
Assembly 1 = Part 1 <1> at d = 25 mm, Part 1 <2> at the default d = 10 mm). Live: behaviours.md drawing toolbar (Update from this
workspace, Ctrl+Q, yellow ↻ "RegenNeeded" when out of date), assembly root-row menu (§ Unfix all), BOM panel (§1280).
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Assembly root row right-click | Properties… · Show instance list properties \| Rename \| Show center of mass · Check interference… \| Create Drawing of … · Add exploded view… \| Export… · **Release…** \| (Add comment) \| Expand all · Collapse all | same, **no Release…** | ✗ CA2 (same gap as the instance menu) |
| 2 | Create Drawing of Assembly 1… › ANSI_A_MM, Suggested views | Top, Front, Right + isometric | Top / Front / Right / iso, hidden edges correct in all three projections, SCALE 1.5:1 | ✅ |
| 3 | BOM table tool | (Insert › "Select a BOM"); fixed-corner choice; click to place | panel "BOM table": Insert [Assembly 1], Template Follow assembly, BOM type Flattened, Order, fixed corner; placed with 2 rows (qty 1 each) | ✅ / ? (picker step) |
| 4 | Change configuration… on Part 1 <1>: ▾ › **Reset to default** | inputs go back to the defaults | **nothing happens** (d stays 25) | ✗ **CA5** (skb) |
| 5 | Reopen Change configuration | d shows "25 mm" | d shows **"25"** (unit lost since it was typed as 25 + Tab) | ✗ CA4 |
| 6 | Type "10 mm" + Enter, ✓ | instance back to the default size | ✅ (Generate stays greyed after typing; it enabled for a List change) | ✅ / ? Generate |
| 7 | Assembly BOM with both instances now at the default configuration | one row, Quantity 2 (same part, same configuration) | **still two rows, Quantity 1 each** | ✗ **CA6** (skb; live check requested) |
| 8 | Switch to the drawing | toolbar Update turns yellow (RegenNeeded), views not yet updated | yellow Update button + a bottom-left hint **"Out of date — Update (Ctrl+Q)"**; views unchanged | ✅ / ? (hint text) |
| 9 | Ctrl+Q | views and BOM table regenerate | all four views redrawn at the new heights; BOM table still 2 rows (= assembly, CA6) | ✅ |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
Live answered "qa5 pass 7" (5)–(6): BOM merges; after a change the only sign is the Update tool turning saturated yellow; the view keeps the old geometry until Update; no text hint.
| Row | Ours | Status |
|---|---|---|
| CA5 / CA6 | fixed (see workflow-configured-assembly.md) | ✅ |
| D17 out-of-date hint | no "Out of date" text now ✅; but after inserting a third instance the drawing **regenerated on its own** at the tab switch (Update stayed pale) | ✗ **D17** (asb) |
| D18 (new) drawing BOM table | shows **2 rows (Qty 1, Qty 2)** while the assembly BOM shows **1 row Qty 3**; Ctrl+Q doesn't change it | ✗ **D18** (asb) |
