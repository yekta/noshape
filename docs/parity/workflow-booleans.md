# Workflow: multi-part booleans in a Part Studio (Boolean union/subtract/intersect, Keep tools, names, Split, Transform copy, Delete part, merge scope)

qa5, 2026-10-22 pass 11 (Split onward finished 10-22 00:30–00:40 by the new qa5 thread), app /tmp/qa5/noshape-0014 (target-shared 00:14, main 5bd50b2).
Doc /tmp/qa5/d25/bool.noshape (setup /root/qa5-scratch/pass11/mkbool.py; clean copy bool-base.noshape): "qa5 bool" = Part 1 block (0,0)–(40,20)×10,
Part 2 cylinder Ø10 centred (40,10) ×20 (New, overlaps Part 1's x=40 face), Part 3 block (60,0)–(80,20)×10. Current doc state: no booleans, Part 2 renamed "Pin", Plane 1 = Right offset 20 mm.
Volumes via /root/qa5-scratch/pass11/q.sh DOC (Ctrl+S in the app, CLI query_parts on a copy). Shots /tmp/qa5/shots/b*.png.
Live: behaviours.md dialog field lists (~329), Transform (~226), Copy here… (~289), Remove/merge scope (~309); request "qa5 (10-22 pass 11, workflow 25 "multi-part booleans" …)" filed (results/names/colours not captured yet).
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Boolean dialog (Alt+C "boolean") | [Union \| Subtract \| Intersect] · Tools · ☐ Keep tools | same (b01) | ✅ |
| 2 | Union Part 1 + Part 2 | ? | one part "Part 1" in Part 1's colour, 9178.097245 mm³ (analytic 9178.097), Parts (2) (b02, b03) | ✅ volume; name/colour ? |
| 3 | Subtract: tools Part 2, targets Part 1 | ? (field labels asked) | fields Tools, Targets, ☐ Offset, ☐ Keep tools; Part 1 = 7607.300919 (analytic 7607.301); ☑ Keep tools keeps Part 2 (b04–b07) | ✅ volumes; labels ? |
| 4 | Intersect Part 1 + Part 2 | ? (is Keep tools offered?) | 392.699082 ✓; **☑ Keep tools is ignored** (tools not kept); switching an existing Subtract to Intersect drops the Targets (b08–b13) | ? BOOL2 |
| 5 | New Boolean after one with ☑ Keep tools | ? | opens with ☑ Keep tools remembered (b12) | ? |
| 6 | **Union Part 1 + Part 3 (disjoint; Part 1 overlaps Part 2)**, Keep tools ☑ or ☐ | live (mesh case, 943): disjoint union reports OK and leaves both parts | preview and commit: **Part 2 (not picked) disappears and a new "Part 4" (4000 mm³, a duplicate of Part 3 in place) appears**: parts Part 1 8000 / Part 4 4000 / Part 3 4000 (b14–b17). Union Part 2 + Part 3 (disjoint) is fine (b18) | ✗ **BOOL1** (kern/psb, severe) |
| 7 | Rename Part 2 → "Pin", Union picking Pin first, then Part 1 | ? | result named "Part 1" (b19, b20) | ? |
| 8 | Split dialog | [Part \| Face] · Parts, surfaces, or curves to split · Entity to split with (+MC) · ☐ Keep tools · ☐ Trim to face boundaries · ☑ Keep both sides | same (b22) | ✅ |

| 9 | Split Part 1 by Plane 1 (x=20), ☑ Keep both sides | ? (names/colours/order asked) | preview: Parts (4); commit: **Part 1** = x>20 half (Pin side), new **Part 4** = origin side, both 4000 mm³, both keep Part 1's colour; list order Part 1, Part 4, Pin, Part 3 (b31–b35) | ✅ volumes; names/order ? |
| 10 | Split, ☐ Keep both sides | ? | a flip arrow appears next to the checkbox; keeps the +X side (plane normal), flip keeps the other side, Parts (3) (b36, b37) | ✅ (side ?) |
| 11 | Parts list › right-click Part 3 | Rename · Properties… · Assign material… · Edit appearance… \| Copy here… · Copy Part 3 \| Create Drawing… · Export… · Release… \| Hide · Isolate… · Make transparent… \| Add comment \| Zoom to selection \| Delete… (behaviours ~221) | same, without Add comment (cloud, excluded) (b39) | ✅ |
| 12 | Copy here… → Transform dialog | Transform 1 · Entities "Part 1" · Translate by XYZ · X/Y/Z · ☑ Copy part (part-studio/102) | identical layout (cmp-b41: ours left, live right) | ✅ |
| 13 | X = 30, ✓ | ? (copy name/colour asked) | row "Transform 1"; copy **"Part 4"** 4000 mm³; colour = the rotation's Part 4 grey `#eaeaea`, **not** Part 3's blue (b41, b42) | ? COPY1 (colour) |
| 14 | Parts list › Delete… on Part 3 | ? (asked: dialog or direct) | commits **"Delete part 1"** at once, no dialog; Parts (3) Part 1, Pin, Part 4 (b44) | ? |
| 15 | Edit Delete part 1 | ▾ Delete composite parts · Entities to delete chip **"Part 1"** = the deleted part's name (part-studio/577) | same fields, but the chip reads **"Part"** (no number) (b45) | ✗ **DEL1** (ui) |
| 16 | Extrude Add, Sketch 4 rect (30,5)–(70,15) on Top (overlaps Part 1, Pin and Part 3), 10 mm | Merge scope is pre-filled with the touched part(s), ☐ Merge with all (part-studio/234: Remove over Part 1 → "Part 1") | **Merge scope empty** (b52) | ✗ **BOOL4** (psb) |
| 17 | …commit with the empty scope | ? (exact default asked) | the whole tool is unioned into **every** part separately: Part 1 11000, Part 2 (Pin) 4785.398163, Part 3 7000 → three overlapping parts, z-fighting stripes where they overlap (b53, b54) | ✗ **BOOL3** (kern/psb, severe) |
| 18 | ☑ Merge with all | Merge scope hidden; tool + every touched part → **one** part, 14785.398163 mm³ (analytic) | Merge scope hidden ✓; result the same 3 overlapping parts 11000 / 4785.398163 / 7000 (b55, b56) | ✗ BOOL3 |
| 19 | ☐ Merge with all, scope Part 1 + Part 3 | Part 1 ∪ tool ∪ Part 3 → one part (Parts (2)) | tool unioned into each: Part 1 11000, Part 3 7000, Pin untouched 1570.796327; still Parts (3) (b57, b58) | ✗ BOOL3 |

Doc for rows 16–19: /tmp/qa5/d25b/merge.noshape (setup pass11/mkbool2.py = mkbool + Sketch 4).
BOOL1 (row 6) and BOOL3 look like the same root cause: a union with several targets is done target by target instead of as one union.

## Update from live's answer "qa5 booleans" (behaviours.md ~7190, part-studio/1601–1644), 10-22 ~01:20
Re-checked rows 2, 4 and 6 on /tmp/qa5/noshape-0014 (doc /tmp/qa5/d25c/b.noshape, shots b60–b62). Final verdicts:

| # | Live | Ours | Status |
|---|---|---|---|
| 1 | Tools field header has a ⇅ icon; empty dialog title red | ⇅ present (b60) | ✅ |
| 2 | Union 1+2 → "Part 1" (Part 1's id/colour) 9178.09724509617, Parts (2). **Default names are positional**: old Part 3 is now listed as "Part 2" | volume/name/colour ✅, but old Part 3 still reads **"Part 3"** (b60) | ✗ **NAME1** (psb): "Part N" = Nth in list order, renumbered on every change (user-renamed parts keep their names) |
| 3 | Subtract fields Tools, Targets, ☐ Offset, ☐ Keep tools; 7607.30091830128; Keep tools keeps Part 2; Tools only → red "Boolean 1 did not regenerate properly: Cannot resolve entities." | same fields/volumes | ✅ (Tools-only error text not re-checked) |
| 4 | Intersect → a **new body** (new id, next palette colour #eaeaea) 392.699081698724, listed **second** ("Part 2"; old Part 3 becomes "Part 1"); ☑ Keep tools → Parts (4): 1, 2, 3 unchanged + "Part 4" = the intersection | result stays **"Part 1"** in Part 1's colour, listed first (b61); ☑ Keep tools ignored | ✗ **BOOL2** confirmed + **INT1** (psb) |
| 5 | Keep tools is sticky across new Booleans | same | ✅ |
| 6 | **BOOL1:** Union Part 1 + Part 3 (disjoint, Part 2 overlaps Part 1 unpicked): blue bubble "ⓘ Boolean resulted in no geometry change. The parts either do not intersect or are totally contained." while the dialog is open, ✓ green; after ✓ row black, no toast, **Parts (3) all unchanged** (not joined into one multi-lump part) | still: Part 2 vanishes and a duplicate "Part 4" of Part 3 appears; no bubble (b62) | ✗ **BOOL1** (psb, severe) + bubble missing |
| 7 | **The first-picked tool survives** (name, id, colour): Pin then Part 1 → "Pin" in Pin's grey | Pin first → "Part 1" | ✗ **NAME2** (psb) |
| 9 | Split ☑ Keep both sides: **both halves get new ids**, keep Part 1's colour, and are listed **at the end** ("Part 3" = x 20..40, "Part 4" = x 0..20; the others move up and renumber) | halves listed first ("Part 1" = x 20..40, "Part 4" = x 0..20 right after it), colours ✅ | ✗ **SPLIT1** (psb, list order + positional names) |
| 10 | ☐ Keep both sides keeps x 20..40 (+normal side), listed last | keeps +X side ✅ (list position as in row 9) | ✅ side |
| 11 | Part right-click with several parts: … \| **Hide, Hide other parts, Hide all parts**, Isolate…, Make transparent… \| … (1624) | **Hide other parts / Hide all parts missing** (b39) | ✗ **MENU1** (ui) |
| 13 | Copy here… copy "Part 4" **keeps Part 3's colour #3b61b4** | grey #eaeaea (next palette colour) | ✗ **COPY1** (psb) |
| 14 | Parts list › Delete… **opens the "Delete part 1" dialog** (▾ Delete composite parts \| Dissolve composite parts \| Ignore composite parts; Entities to delete = Part 3, prefilled; preview Parts (2)); ✓ commits | commits at once, no dialog | ✗ **DEL2** (ui) |
| 15 | chip "Part 3" | chip "Part" | ✗ **DEL1** (ui) |
| 16 | **Default merge scope EMPTY** (correction: not pre-filled); Extrude opens on New, 25 mm; after Add the empty Merge scope field is **pink**, the title red and the preview red | empty ✅, New/25 mm ✅, but **no pink field / red title / red preview** (b52) | ✗ **BOOL4** (revised: error state, not pre-fill) |
| 17 | ✓ with the empty scope → row red "Extrude 4 did not regenerate properly: No merge scope selected.", parts unchanged | tool unioned into every part separately (3 overlapping parts) | ✗ **BOOL3** (psb, severe) |
| 18 | ☑ Merge with all hides the scope → ONE part "Part 1" 14785.3981633975, Parts (1) | scope hidden ✅; 3 overlapping parts 11000 / 4785.398163 / 7000 | ✗ BOOL3 |
| 19 | Scope Part 1 + Part 3 → ONE part "Part 1" 14000 (x 0..80); the cylinder stays separate 1570.79632679489, Parts (2). Scope Part 1 only → Part 1 11000, Part 3 separate | Part 1 11000 + Part 3 7000 (tool merged into each), Pin untouched | ✗ BOOL3 |

Withdrawn: none. Corrected: BOOL4 (live default is empty; the bug is the missing error state + BOOL3's behaviour on ✓).
Open for psb: BOOL1, BOOL3 (severe), BOOL2, BOOL4, INT1, NAME1, NAME2, SPLIT1, COPY1; for ui: DEL1, DEL2, MENU1.

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
Docs /tmp/qa5/d25c/b2.noshape, /tmp/qa5/d25d/s.noshape, /tmp/qa5/d25b/merge.noshape (rebuilt with pass11/mkbool2.py). Shots r01–r25.
| Row | Ours | Status |
|---|---|---|
| BOOL1 disjoint Union Part 1 + Part 3 | Parts (3) unchanged (8000 / 1570.796327 / 4000), message "Boolean resulted in no geometry change. The parts either do not intersect or are totally contained." | ✅ geometry **fixed**; ✗ **BOOL1b** (psb/ui): the message sits **inside the dialog** (live: blue bubble at the top centre of the graphics, dialog without message) and the committed row's hover tooltip repeats it (live: just "Boolean 1") (r01–r03) |
| NAME1 positional names | after Union 1+2 old Part 3 is listed "Part 2" | ✅ **fixed** (r04) |
| NAME2 first pick survives | Part 2 then Part 1 → grey cylinder body survives, 9178.097245 | ✅ **fixed** (r05) |
| BOOL2 Intersect ☑ Keep tools | Parts (4): 8000 / 1570.796327 / 4000 / Part 4 = 392.699082 | ✅ **fixed** (r09) |
| INT1 Intersect ☐ Keep tools | new grey body listed second "Part 2" 392.699082; old Part 3 → "Part 1" | ✅ **fixed** (r10) |
| (new) Intersect of **disjoint** Part 1 + Part 3, ☑ Keep tools | creates an extra **"Part 4" of 8000 mm³** (a copy of Part 1), Part 1 recoloured grey | ✗ **INT2** (psb; live not captured, but an empty intersection can't be 8000 mm³) (r06, r07) |
| SPLIT1 | halves listed at the end: Pin, Part 2 (copy), **Part 3 = x 20..40, Part 4 = x 0..20**, both Part 1's colour | ✅ **fixed** (r16, r17) |
| COPY1 | Transform copy keeps Part 3's blue | ✅ **fixed** (r11) |
| DEL1 | chip "Part 3" | ✅ **fixed** (r12); dropdown Delete / Dissolve / Ignore composite parts ✅ (r13) |
| DEL2 | Parts list › Delete… opens "Delete part 2" dialog, preview Parts (2) | ✅ **fixed** (r15) |
| MENU1 | Hide, Hide other parts, Hide all parts present | ✅ **fixed** (r14) |
| BOOL4 empty merge scope | empty pink "Merge scope", red title, red preview ✅; but ours also shows a blue bubble **"Selected tools and targets do not intersect."** and the **✓ is disabled** (live: ✓ allowed → red row "Extrude 4 did not regenerate properly: No merge scope selected.") | ✗ **BOOL4b** (psb, minor) (r20, r21) |
| BOOL3 ☑ Merge with all | ONE part "Part 1" 14785.398163, 17 faces | ✅ **fixed** (r22) |
| BOOL3 scope Part 1 + Part 3 | ONE part 14000 + cylinder 1570.796327 separate | ✅ **fixed** (r24) |
| BOOL3 scope Part 1 only | Part 1 11000, Part 2 1570.796327, Part 3 4000 | ✅ **fixed** (r25) |

## Re-run pass 19 (2026-10-22 ~11:00–11:40, app /tmp/qa5/noshape-1013 = /root/target-lead/debug/noshape 10:13: the queue's 09:58 batch build = main 3ce6952 + the ready branches of that batch; CLI 08:11)
| Row | Ours | Status |
|---|---|---|
| BOOL1b no-change message | blue bubble at the top centre of the graphics, dialog without message (z05) ✅; committed row tooltip **"Boolean 1 <- Extrude 1, Extrude 3"** (live: just "Boolean 1") (z06) | ✅ bubble / ✗ tooltip (minor) |
| INT2 disjoint Intersect ☑ Keep tools | Parts (3) unchanged (8000 / 1570.796327 / 4000) | ✅ **fixed** |
| BOOL4b empty merge scope | ✓ enabled, no extra bubble; ✓ → red row "Extrude 4 did not regenerate properly: No merge scope selected.", parts unchanged (z09, z10) | ✅ **fixed** |
