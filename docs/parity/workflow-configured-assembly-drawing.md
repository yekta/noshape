# Workflow: drawing of a configured assembly with a BOM table, after a configuration change

qa5, 2026-10-21 pass 8, app /tmp/qa5/noshape-1924 (target-shared 19:24), doc qa5-cfgasm.noshape (see workflow-configured-assembly.md:
Assembly 1 = Part 1 <1> at d = 25 mm, Part 1 <2> at the default d = 10 mm). Live: behaviours.md drawing toolbar (Update from this
workspace, Ctrl+Q, yellow ↻ "RegenNeeded" when out of date), assembly root-row menu (§ Unfix all), BOM panel (§1280).
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Assembly root row right-click | Properties… · Show instance list properties \| Rename \| ✅ (Release… present now; = live) | 77870dd4 10-28 | Export… · **Release…** \| (Add comment) \| Expand all · Collapse all | same, **no Release…** | ✗ CA2 (same gap as the instance menu) |  |
| 2 | Create Drawing of Assembly 1… › ANSI_A_MM, Suggested views | Top, Front, Right + isometric | Top / Front / Right / iso, hidden edges correct in all three projections, SCALE 1.5:1 | ✅ | 77870dd4 10-28 |
| 3 | BOM table tool | (Insert › "Select a BOM"); fixed-corner choice; click to place | panel "BOM table": Insert [Assembly 1], Template Follow assembly, BOM type Flattened, Order, fixed corner; placed with 2 rows (qty 1 each) | ✅ / ? (picker step) | 77870dd4 10-28 |
| 4 | Change configuration… on Part 1 <1>: ▾ › **Reset to default** | inputs go back to the defaults | **nothing happens** (d stays 25) | ✗ **CA5** (skb) → fixed later: CA5 ✅ 7e88e3ab |  |
| 5 | Reopen Change configuration | d shows "25 mm" | d shows **"25"** (unit lost since it was typed as 25 + Tab) | ✗ CA4 → fixed later: CA4 ✅ 77870dd4 (Blur / Enter keep the unit) |  |
| 6 | Type "10 mm" + Enter, ✓ | instance back to the default size | ✅ (Generate stays greyed after typing; it enabled for a List change) | ✅ / ? Generate |  |
| 7 | Assembly BOM with both instances now at the default configuration | one row, Quantity 2 (same part, same configuration) | **still two rows, Quantity 1 each** | ✗ **CA6** (skb; live check requested) → fixed later: CA6 ✅ (v0.4.12 CLI: merged rows) |  |
| 8 | Switch to the drawing | toolbar Update turns yellow (RegenNeeded), views not yet updated | yellow Update button + a bottom-left hint **"Out of date — Update (Ctrl+Q)"**; views unchanged | ✅ / ? (hint text) | 77870dd4 10-28 |
| 9 | Ctrl+Q | views and BOM table regenerate | all four views redrawn at the new heights; BOM table still 2 rows (= assembly, CA6) | ✅ | 77870dd4 10-28 |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
Live answered "qa5 pass 7" (5)–(6): BOM merges; after a change the only sign is the Update tool turning saturated yellow; the view keeps the old geometry until Update; no text hint.
| Row | Ours | Status | Verified |
|---|---|---|---|
| CA5 / CA6 | fixed (see workflow-configured-assembly.md) | ✅ |  |
| D17 out-of-date hint | no "Out of date" text now ✅; but after inserting a third instance the drawing **regenerated on its own** at the tab switch (Update stayed pale) | ✗ **D17** (asb) |  |
| D18 (new) drawing BOM table | shows **2 rows (Qty 1, Qty 2)** while the assembly BOM shows **1 row Qty 3**; Ctrl+Q doesn't change it | ✗ **D18** (asb) → fixed later: D18 ✅ fixed (row below) |  |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| Row | Ours | Status | Verified |
|---|---|---|---|
| D18 drawing BOM | 1 row Qty 3 = assembly | ✅ **fixed** (c31) |  |
| D17 drawing after a new instance | Update saturated yellow, views/BOM unchanged until Ctrl+Q; Ctrl+Q → Qty 4 | ✅ **fixed** (c33, c34) | 77870dd4 10-28 |

## Re-check (qa4, 2026-10-23 20:40–21:10, main a0e360a3; code and the latest qa5 passes)
- CA5, CA6, D17, D18 fixed (rows above). CA4 ("25" + Tab loses the unit): fixed in code (assembly_panels.rs: Blur / Enter
  commit with the workspace unit, "25" → "25 mm"), listed fixed in qa5.md. CA2: cited in code. No open rows.
