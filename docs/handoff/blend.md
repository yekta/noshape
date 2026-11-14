# Blends (agent "blend")

## 11-14 08:30 UTC: handoff (Bbedward's week ran out): queued, running where, next

Landed since 03:50: f2twice (2eb4cf4b, dx's P0: pipeflange Fillet 2 built twice, 356 -> 157 ms; dx told).

Land queue (`tools/tiers/land.sh --status` shows only the items of the worktree it runs in: run it in each):
- blend5 `blend-coaxdirect` 23ab8f4c: coaxial rounds built directly (snap-hook Fillet 5 ~66 -> ~20 ms). Checking.
- blend6 `blend-raysfirst` 91cb6945: the direct build's tool classification by rays first (2 commits). Queued.
- blend7 `blend-p2live` 201b4e5e: variable + partial fillets as live 3028 (3 commits). Queued.
  Their asm runs all show asm-tvc-mount 42 -> 43/43 occurrences, 4 -> 3/10 mates against the runs before 11-14 01:00;
  identical on three independent branches, so it is main's (likely skasm's 7ce8b87ee translator change), not ours.
  If one fails, fix forward in its worktree and land.sh again.

Gated, waiting on its last runs (systemd units on this machine, `systemctl list-units 'blend-*'`; flags in
/root/blend-scratch/*.done, logs there):
- blend4 `blend-secwires` (2 commits on main 618a5fbc3: hold lines sampled once per edge with regula-falsi crossings
  ae879b804, then a round's loft sections as wires 8dc943662). Pair on vibe7 clean (870 = 870, faceblend 388 -> 169 ms,
  Face blend 1 346 -> 130). Unit blend-sec-gates7: asm on vibe7 (asm-secwires-new.log -> secwires-asm.done), then a
  timing alternation (alt-secwires.sum). Land both commits together when the asm shows nothing worse than
  /root/blend-scratch/asm-p2.sum (rebase on main, land.sh from blend4). blend10's `blend-holdcache` is the first commit
  alone: don't land it separately.
- blend2 `blend-partrays` 40c1654d5 (on raysfirst): the part's classification by rays first too. Unit
  blend-partrays-pair7: pair on vibe7 (base blend3 = raysfirst detached). After rays lands: rebase on main, asm
  (one per 8-core host: earlyoom), measure c2-rings-loft Chamfer 1 (classify.part 42 points), land.

Scratch: blend-base holds a debug-only patch (ZZ per-pair section timing in blend_direct.cpp, never land it); its
run (pairs.log) times each part-face x blend-face section of cf-snap-hook and c2-hunting-knife.

Next (speed, live's per-feature times on the Mac release build):
- cf-snap-hook Fillet 6 (32 vs 8 ms): ~55% is blend_direct's sections (part faces x blend faces, GeomInt_IntSS in
  parallel). pairs.log (08:40, vibe6 loaded): all 74 pairs come back with no curve; cone x torus 40 pairs 562 ms of the
  763 summed, cone x cylinder 8 pairs 145, cone x B-spline / plane x B-spline ~27 each, plane x torus 2. Skipping
  them needs an exact proof of no curve (lead: boxes and face domains only; parked/sections-box-domains-wip wasn't
  enough): e.g. the cone face's distance to the torus's spine circle beyond the tube radius + tolerance, checked
  cheaper than the IntSS it saves. **Done (lead 08:45): `blend` worktree, branch `blend-emptysec` 2b4b6432f**
  (`misses_by_spine` in blend_direct.cpp: plane / cylinder / cone part faces against torus or cylinder blends, margin
  10 x max(tol, snap) + both faces' tolerances; T1 kernel blend 59/59 and model fillet/chamfer/blend/round 70/70).
  Unit blend-emptysec-alt6 times snap-hook, knife, rings against blend8 (detached main 2702712c7) on vibe6; then unit
  blend-emptysec-gates6: pair, asm, g++-13 on vibe6 (emptysec-gates.done). Don't touch `blend` or `blend8` till then.
- c2-hunting-knife Fillet 3 (~90 vs 11 ms): OCCT's as-is attempt 25 ms (then replaced by rolling balls), blend_direct 30,
  lofts + Rust 35. No cheap win: running the balls beside OCCT would build them for every fillet, contending for the
  kernel lock. Chamfer 1 (165 vs 19) is bool's Rust Boolean (sections_ahead) plus 20 ms of scaled retries; Fillet 2
  (70 vs 10) is OCCT once plus the Rust Boolean of its run-on.
- c2-tpd-fillet-variable Fillet 1 is the variable round's tool cut by bool's Rust Boolean (sections_ahead ~80%): bool's.
- c2-variable-fillet-faceblend after secwires: 8 lofts (parallel) and blend_direct's B-spline sections remain.
- Live answers pending: 3022 part 2 (knife Fillet 7's mid-edge radius; ours takes 11.85 mm³, live 7.21) and batch 2
  (FILLET-VAR-SECTION, VAR-CORNERS, PARTIAL-DIR, ASYM-SIDE, FACEBLEND-HOLD-INVERSE) in live/REQUESTS.md.

Hosts: model pairs and asm on vibe6/vibe7, one asm per host (earlyoom; check `free -g`); never vibe5 for long runs (a
guard there SIGTERMed my jobs at load 45; it hosts the land queue); the Mac for short builds and crate tests only (one
shared slot). Scripts in /root/blend-scratch/bin: host-pair.sh, new-side.sh, asm-one.sh (asm-host.<worktree>
overrides the host; "skip" + ASM_FORCE), asm-only.sh (the assembly suite has no name filter: it runs everything),
alt-rel.sh (alt-host overrides). Remove a landed branch's targets on vibe5/6/7 (`~/agents/<wt>/target`).

## 11-14 03:50 UTC: landed overnight, gates in flight, hosts

Landed: f197 P0 (e1fc6879), torus rounds (fab048ae, knife Fillet 5 as live), the fillet's drag arrow as std's
(b2d2b2c4b), a variable fillet of a tangent run in one build (33873848d: knife Fillet 7 symmetric, live's 25 faces;
volume 1474.19 vs live 1478.82, we take 11.85 mm³ where live takes 7.21: waiting for live's mid-edge radius, 3022 part 2).

In flight (local branches; every pair so far 870 = 870 with no differences):
- blend-f2twice (blend8): pipeflange Fillet 2 was built twice since 6d4ac9879 (133 -> 510 ms, dx's P0); pair on vibe5.
- blend-coaxdirect (blend5): coaxial rounds built directly (snap-hook Fillet 5 ~66 -> ~20 ms); asm on vibe7.
- blend-raysfirst (blend6): the direct build classifies against tools by rays first (knife Fillet 3 94-102 -> 88-90 ms);
  asm on vibe7 after coax (the vibe6 run was OOM-killed at load 50).
- blend-p2live (blend7): variable + partial fillets as live 3028 (pair clean, g++-13 OK); asm after rays.
- blend-partrays (blend2, on raysfirst): the part's classification by rays first too (rings Chamfer 1: 42 points).
- blend-holdcache (blend10): hold lines sampled once per edge, crossings by regula falsi (faceblend Face blend 1
  423 / 402 -> 272 / 301 ms on a loaded vibe7: 121725 edge points before).
- blend-secwires (blend4, on holdcache): a round's loft sections as wires (212 section faces).

Dropped: building fillet_as_cut's tools from their sections and blend_direct before the OCCT cut: slower on the
groove test (19.8 -> 30 ms), and c2-tpd-fillet-variable's Fillet 1 doesn't use it anyway: it is the variable round's
tool cut by bool's Rust Boolean (boolean.rust.sections_ahead ~80% of the feature): bool's.

Hosts (lead 11-14): model pairs on vibe5/vibe7 (host-pair.sh, new-side.sh reuses a same-host base log), never the Mac
(short builds only, one slot). vibe6 is shared with psui/studios and ran out of memory with 3 of my jobs. perf isn't
allowed on the vibe hosts (perf_event_paranoid 4): find hot calls with NOSHAPE_KERNEL_PROFILE (gcc names the bridge calls).

## 11-13 16:40 UTC: knife Fillet 2 / 3 faster (blend-speed)
- A fillet of straight edges with overflow on tries OCCT once without its scaled retries before its sections
  (`k::without_blend_retries`): c2-hunting-knife's Fillet 2 ~300 -> ~150 ms, results identical.
- OCCT's attempt whose round splits is returned as built (`k::with_blend_split_raw`, features.cpp `scaled_blend`: no
  finishing passes, ~190 -> ~60 ms) for the rolling-ball route that replaces it; the balls' faces oriented by their
  normals (no classifier); a section per 0.1 rad: Fillet 3 ~390 -> ~320 ms. Gates: model suite 863 = 863, assemblies
  identical (1518 / 408), g++-13.

## From psui (lead: fillets are blend's; 11-13 14:01 UTC): an over-large fillet regenerates "Ok" into a broken body
The 40 × 30 × 10 block (ui-parity start "block"), Fillet on the top front edge (`edge through [20, 0, 10]`): 20 mm fails
as live (ui-parity fillet-too-big), but **100 mm regenerates Ok into a body with no faces**; on a vertical edge 100 mm
gives Ok and a 5-face body. Live (feel/feedback.json "failedPreview", live2 11-13): "Fillet 1 did not regenerate
properly: Failed to fillet selections.", no preview body. part-studio-ui's feedback matrix holds it as a gap ("error /
a failed preview"); it moves to matched (and fails the test until its gap flag is flipped) once the fillet errors.

Area (lead 11-07): fillets, chamfers and face blends: noshape's own blends (crates/brep/src/blend*, rust_blend.cpp,
ticket BLEND-RUST), OCCT's blend route and its patches, and the blend features in crates/model-core
(regen/features/detail.rs `fillet` / `chamfer` / `variable_fillet`, blend_section.rs, faceBlend). core keeps the rest
and reviews blend changes on request. History before 11-07: docs/handoff/core.md ("noshape's own blends",
"Mixed-radius corners", the 11-07 states).
Worktree /root/projects/noshape-wt/blend (and blend2 for a second build), builds on vibe3 since 11-10 (lead:
`NOSHAPE_RB_HOST=vibe3 tools/tiers/rb.sh ...`; suites: /root/blend-scratch/bin/suite-pair.sh / asm-pair.sh <tag> vibe3), scratch
/root/blend-scratch. Any change in crates/brep passes bool's gate first (handoff/bool.md: every corpus and parity56 at
0 differences from OCCT).

Parity runs: `NOSHAPE_RB_HOST=vibe5 tools/tiers/rb.sh env NOSHAPE_PARITY_MODELS=/home/ngyekta/parity/models
NOSHAPE_PARITY_ONLY=<docs> cargo test --release -p noshape-ops --test all parity::parity_models -- --ignored --nocapture`.

**c2-rings-loft stops at feature 20, Extrude 5 (core's):** the engraving cut's edges are split at the revolved ring's
seam (y = 0, below the axis: 7 edges in two pieces, 342/213 edges/vertices vs live 335/206); Parasolid has no seams.

## 11-13 16:30 UTC: fillet speed rows (branch blend-classify on blend-speed)
- Tools lofted bare (`k::loft_tool`, features.cpp `loft_impl(.., bare)`): ThruSections' own surfaces, no history, its
  planar sides as planes and its two end sections' own curves (restored without the whole-solid check); the Loft
  feature's natural surfaces, merging and history took ~75 % of a ball tool's loft. Rounds' section tools too; a
  chamfer's keep the full loft (c2-hunting-knife's Chamfer 1 needs its sides as the Loft feature makes them: 24 -> 15).
- blend_direct: a tool is taken or added by its strip's normal against the part face it lies on (no solid classifier);
  a part piece beside a tool's strip on its own face is in the tool by a 2D test on the strip (only other faces' pieces
  go to the tools' classifiers: the knife's Fillet 3 16 -> 4 calls); blend-against-blend sections in parallel; lofted
  sides' samples off a face's box not projected.
- A rim round's tools and a run-on's face distances in parallel; rim chamfers built directly too (the classification
  cost that kept them on the cut is gone: knife Chamfer 2 ~73 -> ~36 ms, rings Chamfer 1 ~115 -> ~95 ms, Mac dev,
  three alternations).
- Release, Mac, two alternations against blend-speed on the same main (per feature / whole document): knife Fillet 3
  136 -> 89 ms, Chamfer 2 86 -> 34, document 710 -> 602 ms (live 408); rings Fillet 1 50 -> 37, Chamfer 1 149 -> 92,
  document 1248 -> 1156 (live 1557); faceblend Face blend 1 140 -> 89, document 142 -> 94 (live 58); snap-hook Fillet 6
  32.5 -> 29.7, document 350 -> 344 (live 274); michael unchanged (90; its Fillet 2's 84 ms is the Boolean, bool's).
- Dev builds, vibe7 loaded: knife Fillet 3 661-698 -> 280-363 ms, rings Fillet 1 232-302 -> 130-189, snap-hook Fillet 6
  226-237 -> 120, Fillet 3 80-89 -> 50-53, faceblend Face blend 1 407-655 -> 368-456. Results identical (model suite
  863 = 863 twice).
- Tried, no win: the knife's Fillet 2 fuse through blend_direct (slower, one face too many; patch parked/runon-direct);
  snap-hook's empty sections (above). The knife's Chamfer 1 directly (parked/knife-chamfer1-direct): the chamfer's sides
  only approximate the blade's B-spline faces (0.006 mm), their projected touch curves chain across face 5 but don't
  close a region with its boundary (one piece, invalid). Second try (parked/knife-chamfer1-free-tools-wip.patch, switch
  ZZ_CHAMFER_DIRECT): the tool as the Boolean's (corner out in the air), `blend_direct(.., free_tools)` taking a tool that
  lies on no face away by its faces' sections with every part face its box meets, each section's chamfer line run 5 %
  past both faces (so the section crosses them inside the tool's face: at its edges it came in 13 pieces), two free tools
  end to end adding nothing where their shared section won't trace. It now gets through the sections; left: the blade's
  two B-spline faces split badly (face 4 not split by its one curve, face 5's three pieces invalid, one of no area), so
  the shell stays open along the chamfered edge. Face 4 is the blade's 0.25 mm edge face: its one section curve ends
  0.03 mm and 0.3 mm short of its ends, where each tool ends on a section plane tilted to the face's end edge (the
  Boolean's sliver end, collapsed after); the end caps' sections with it put no cut there. Next: those cap x face
  sections (patch parked/knife-chamfer1-free-tools-wip2.patch prints each face's bounds and cuts). wip3: at the
  x -114.3 end the cap lies in the end face's plane (its section runs along face 4's end edge, rightly dropped); the gap
  is the chamfer face's own section with face 4, which stops at x -114.14 (0.3 mm short of the cap).
- Mac (lead 11-13): the model suite takes 12-16 min a side there (build included), vibe7 ~45 min.

## 11-13 15:20 UTC: cf-snap-hook's empty sections (lead: skip only what boxes and domains prove): not a win, parked
- The direct build's part-face x blend sections that keep no cut: on snap-hook, knife, rings and faceblend 186 of 226
  pairs; snap-hook's 64 cone x torus pairs alone ~110-160 ms (dev, Mac). Every one of them has tight (AddOptimal) boxes
  that overlap by mm (snap 0.023 mm): the torus meets the next cone near-tangentially where its end and the cone's base
  meet the plane (IntSS lines 0.01-0.4 mm there, all off one face). Boxes alone prove only 8 cylinder x free-form pairs
  empty (~2 ms).
- Tried (patch /root/blend-scratch/parked/sections-box-domains-wip.patch): each surface's parameters clipped to what can
  reach the two faces' common box (+3e-3 size): planes' and axial coordinates linear, an angle about an axis extreme at
  the box's corners when its shadow misses the axis; an empty range skips the pair. Cuts identical on all 226 pairs
  (endpoints within 1e-6 size), but no faster: IntSS samples a fixed grid whatever the domain, and the near-tangent walk
  stays. Same-run timing on the Mac, both orders: whole 294 / 435 ms, clipped 365 / 386 ms (the second run of a pair
  ~20 % faster either way).
- Mac mini (lead 11-13): `NOSHAPE_RB_HOST=mac`, models in ~/noshape-host/parity/models (copied 11-13). A fresh dev build
  of noshape-ops' tests 4 min 37 s there (vibe5 at 3 jobs: 5-7.5 min incremental); the 4-document probe ~1.5 min.

## 11-13 14:30 UTC: oversized fillets fail as live (psui's 40 x 30 x 10 start block)
- A fillet of R100 on the block's top edge (and R1000 on a vertical edge) regenerated "OK" with a body of no faces: the
  checked blend's `valid_changed` passed a result with no new face, and an empty solid has none. Now `valid_changed` and
  `valid_shape` want a face, and `fillet` / `chamfer` (detail.rs) fail "Failed to fillet / chamfer selections." when a
  body they touched has no solid or no face. T1 `kernel_robustness::an_oversized_blend_fails_as_live_never_an_empty_body`.
- **Settled by live 2986 (18:54 UTC):** the vertical edge builds at every radius to R100 in live, our volumes exactly (R30
  10068.583 / 6 faces, R45 7786.122 / 5, R60 5281.409 / 5, R80 2437.211 / 5, R100 599.887 / 5): no change, psui told.
  Top edge y = 0: live R9.9-R20 as ours (R20 8913.479 / 6), R100 error; x = 0: R15 10593.828, R20 9685.109 as ours.
  Open: live's first failing radius on the top edges (ours builds the roll-over through R60, fails from R80); live
  bisects R25-R80 in the UI (asked 19:00).
- Was (live REQUESTS 14:18): psui reports live fails R100 on the **vertical** edge too, where ours builds the ball's
  roll-over (599.887 mm³, 5 faces: exactly block ∩ the ball's cylinder, the same shape as live's 10-29 R45 on 40 x 20,
  4267.940743, 5 faces, OK). Live's limit for a single edge's roll-over isn't recorded; the request asks R30-R100 here
  and the top edge's R10-R20 (ours: R20 on the 40 long top edge OK, 8913.479 mm³).

## From qa (11-13 04:18 UTC): P3 CHAMFER-SAYS-FILLET (fixed with blend-chamfer2: the prism cut's own failure no longer
replaces OCCT's error)
qa's ticket document cli-draft-hang (vibe5 ~/qa/tickets): Chamfer 3 fails with "Failed to fillet selections." on main 6932701dc
(v0.4.47: "Distance of chamfer on face too large."); a chamfer's failure names the chamfer ("Failed to chamfer selections.",
detail.rs's Overrun::PastUnselected). Since 8ed9c15e4 by its subject; the same change fixed cli-draft-hang's and
cli-suppress-segv's Chamfer 2 (Error -> Ok).

## From bool (11-13 03:55 UTC, lead: documents slower than live, by feature)
Baseline qa's 02:00 vibe7 release run (head 91a90caf); per feature ours from a profiled suite run on vibe5 (loaded: rough),
live's from live.jsonl per_feature_ms. Yours:
- c2-hunting-knife (1.2 s, live 0.4): Fillet 2 ~300 ms (live 10), Fillet 3 ~160 (live 11); Chamfer 1 ~775 (live 19), its
  Boolean bool2's 343b971e.
- c2-rings-loft: Chamfer 1 366 ms (live 30).
- c2-variable-fillet-faceblend: Face blend 1 540 ms (live 25).
- learning-center-michael: Fillet 2 400 ms (live 46).
- cf-snap-hook: Fillet 6 126 ms (live 8).
- c2-impeller-loft: Fillet Ansaugflansch zum Gehaeuse leaves 9 vertices of 1.1-1.8 mm tolerance (bool2's 22:40 note), which
  put the two flange-hole extrudes' Booleans on OCCT (~3.1 and ~1.4 s against live's 177 and 59 ms).
- (11-13 04:03, qa's ranking list) c2-variable-fillet-faceblend (0.39 s vs 0.06): Face blend 1 540 ms (live 25): 8 op_loft
  334 ms, blend.direct 88 ms. learning-center-michael (release 203 vs 96 ms): Fillet 2 400 ms (live 46), blend.by_tools 384
  ms of which our Boolean 283 (bool takes the Boolean's speed).
- (11-13 06:42, bool) The private dock (the user's own document; mode-700 copies on vibe5 ~/tmp/bool/private, never in
  the repo): two vertices at (±21, 18, 1.274) mm come into five Booleans with a 1.195 mm tolerance, 0.4-1.1 mm off
  their planes (as the impeller's 1.1-1.8 mm ones after its fillet): our Boolean declines them and OCCT takes ~580 ms;
  and two vertices of 0.38 mm tolerance near (1.21, 15.2, 0.405) mm put a 1355-face cut (1.24 s) on OCCT. Likely a
  blend upstream; worth checking which feature makes them.
- The private dock (E2 tab): a rib round, op_fillet 1.8 s of its 2.6 s; hat8 / hat15: a chamfer ~460 ms.

## From core (11-13 00:30 UTC, lead): core works on isaac-ball's Fillet 2 in the fillet builder
Where tangent propagation stops (JN1 concave, the 15 mm rounds' bottom arcs convex: likely a convexity change ending
live's chain) and extending the neighbouring rounds into the new fillet. Gate: your suites and corpus (I'll read your
handoff for them); tell me in core.md if you're in the same code (fillet chain / blend_direct) so we don't collide.

## From core (11-13 00:21 UTC, lead: core takes asm-isaac-ball's Fillet 2; it's the fillet itself, so coordinating here)
asm-isaac-ball studio 00f1d4's Fillet 2 (4 mm, tangent propagation, allowEdgeOverflow on, one edge JN1) fails in ours
("Failed to fillet selections."), builds at 3.9 mm; live adds 98.0 mm³ (17064.678 -> 17162.687, 17 -> 18 faces). 29 of
isaac-ball's mates hang on it. **Our input matches live**: after Fillet 1, 17064.678269 mm³ (live 17064.6781), 17/39/24;
JN1 is our r 22.5 arc at z -0.4017, 34.475 mm (live 34.461), its tangent chain the 15 mm rounds' bottom arcs (12.070,
live 12.065) and 2.036 mm lines. **The limit:** JN1 joins the bottom plane z = -0.401684 to the r 22.5 cylinder wall,
which ends at z = -4.401684, exactly 4.000 mm high; a 4 mm round takes the whole wall and its contact line falls on the
wall's lower edge (zero width left), where 3.9 mm leaves 0.1 mm. Live rolls over it (edge overflow). core will try it in
the fillet's overflow / face-consumed handling unless you say you have it or a rule for it; tell me in core.md. Repro:
vibe6 ~/tmp/core/ibm (NOSHAPE_PARITY_MODELS there, parity_models), saved document ~/tmp/core/ibsave/t.noshape
(our feature 6 = live's 5: an In context entity first).
**More (core 00:23):** even below the limit ours isn't live's. The geometry: Extrude 1 an r 22.5 cylinder (bottom
z -4.4); Extrude 2 a tab on its -x side (y +-11, z -0.4..21.6); Fillet 1 the tab's two vertical concave corners (15 mm);
JN1 the concave arc between the tab's underside (z -0.4, facing down) and the 4 mm of cylinder wall below it. Live: one
fillet face (JOC 173.8 mm²) along JN1, the tab's underside 179.4 -> 74.6 mm², the two 15 mm rounds extended down
(265.46 -> 279.04 mm² each), +98.0 mm³, 18 faces. Ours at 3.999 mm: tangent propagation runs on round the rounds' bottom
arcs (B-splines 57.7 mm²), the 2.04 mm lines and the corners (tori 8.1, cylinders 12.8), cuts the tab's side planes
(44.8 -> 36.7) and x -29.06 plane (484 -> 477), the rounds shrink (265.6 -> 239.1), the torus along JN1 only 81.1 mm²:
+21.7 mm³, 24 faces. So live stops the chain at JN1's ends and runs the 15 mm rounds on into the new fillet (Parasolid's
cliff / roll-over), and the 4.000 mm wall is consumed. core can't take this one quickly; it's yours when the knife allows,
unless the lead says otherwise.
## 11-13 02:47 UTC: the knife's Chamfer 2 (branch blend-chamfer2 on blend-knifetip, gates running)
- Live (behaviours.md "c2-hunting-knife, Chamfer 2's hole-rim chamfers", 01:04): the one entity is the hole wall
  (a face: both rims), Offset, 0.75 mm and 45°; JiC (y > 0) 18.856481, JiG (y < 0) 19.444938 mm². **The rule that
  fits every contact distance live gave** (knife/ch2sim.py in blend-scratch): constant offsets from the wall and the
  side, sized so that at the rim's middle the wall carries 0.75 and the chamfer 45° from it; the middle is where
  Parasolid's ellipse is halfway from its start, and the two rims start on opposite sides (JhR at x 27.5, JhV at
  32.5: the major axis's end the hole's axis points to on the side), so through sides tilted 0.5° the two rims
  differ. Distance on the side instead, sizing at the start, or the asked 0.75 as an offset: none fit.
- Built as `ring_blend`'s rim chamfer with unequal offsets (`RimBlend::Offset`); a picked cylinder's seam dropped
  from a face pick. Ours 18.869078 / 19.458019 (OCCT's constant setbacks: 19.161 each): ~1 µm wider on the side than
  live's (its plane contacts 20.332 / 20.497 mm, ours 20.339 / 20.504), within the 0.19 % band. T1 features_c::
  an_offset_chamfer_of_a_holes_two_tilted_rims_sizes_each_at_its_own_middle_as_live.
- **Open:** (1) which face carries the distance: here the picked wall, but a pin's picked top measures Distance 1
  down its sides (features_a pins test), so the picked-wall rule is only in the rim route; live probe asked
  (REQUESTS 02:47). (2) Parasolid's ellipse start: the knife's rims start at the end the axis points to (our axis =
  Parasolid's +Y), the rings' equal-chamfer rule (2441, `Rim::whole_middle`) takes the other end with axes of ours;
  kept apart until a case settles it.
- asm-hand-drill-pinslot's studios (as models in vibe5 ~/tmp/blend/hd): 9fbc05's Fillet 1 (0.3 mm on 231 edges of a
  33-tooth spur gear) fails after 31 s, live builds it in 1 s (dump blend-scratch/hd/fillet-5.*); f79298's Chamfer 1
  likewise; 0d1e01 / 4bed3f first differ at extrudes. Next after the knife.
- c2-impeller-loft's Fillet Ansaugflansch zum Gehaeuse (core's tolerant vertices): OCCT's round along Delete face 2's
  degree-14 B-spline outline comes out in 7 faces with 1.14 mm vertices; but the part differs from live from feature 15
  on (Boolean Flansch+Gehäuse 15/41/30, live 19/56/39), so it waits for that.

## 11-12 23:06 UTC: the knife's Fillet 3 as live (branch blend-knifetip a9e4c9369, pairs queued on vibe7)
- **Rolling balls** (`blend_section::rolling_ball_blend`, detail.rs `fillet`): when OCCT's fillet leaves one edge's round
  in several faces (`split_blends`: c2-hunting-knife's tip pinched at its apex, its corners split where the round runs
  past the chain's open end), the chain is rebuilt of exact balls, one tool per edge (`ball_tools`): each section's
  ball solved on the true faces (`Support`: planes and cylinders run on past a free end; other faces as they are), its
  sides on the faces to the edge's point (lines on planes, arcs on curved faces) so the direct build takes them as
  strips; lines between planes one section extruded (live's cylinders); lofts with a section per 0.05 rad of turn;
  where two edges meet one shared ball, the one touching the edge between the faces that change (live's circle KjpB:
  19-44 um off the vertex's plane, which moved 0.015-0.034 mm2 between faces); past a free end the ball rolls on until
  off the part. Built by `k::blend_direct` only (a boolean by these tools fails or leaves slivers: declined, OCCT's kept).
- Contacts between a plane and a cylinder are ellipses (`plane_cylinder_contacts`; kernel `canonical_edges` rebuilds
  the B-spline edges on them): live's 12 ellipses on the knife.
- c2-hunting-knife 20 -> 24 of 52; Fillet 3's faces vs live: tips 7.829009 (7.829014), corners 1.003963 / 1.004665
  (1.004015 / 1.004703), coves 10.855977 (10.855988), straight rounds equal to 6 decimals; live's corner grids (out2.json)
  within 0.06 um inside the faces. Fillet 3 ~620 ms (live 11; OCCT's attempt ~110-170 of it, the tools built in
  parallel).
- **The knife's next difference (feature 24, the first with live volumes), not blend's:** volume 9301.30 mm3 vs live
  9306.25, 3 lines that are live's B-splines; the centroid shift puts the missing volume at x ~ -84 (the handle), and
  the handle's loft face (live JZa 2443.957 mm2, ours 2443.869) is ~2 um inside live's: Loft 3 / Bridging curve 1.
- **Chamfer 2 (mine, next):** live's two rim chamfers of the x = 30 hole differ (JiG 19.444938, JiC 18.856481 mm2;
  ours 19.160723 each): an Offset and angle chamfer's offset face seems to follow each edge's direction, ours follows
  face order. Live capture requested (REQUESTS.md 22:57).

## 11-12 20:28 UTC: impeller Fillet 1 speed, rings-loft Chamfer 3, the knife's Fillet 3 (michael parked)
- blend-impspeed (queued 3c52dc84): the tube-end round splits its faces in 2D (`k::split_on`, BRepFeat_SplitShape: each
  touch curve projected onto its face, cut where it crosses a seam, ends shared) and checks its touches by (u, v)
  (`Shape::contains_uv`): c2-impeller-loft's Fillet 1 ~657 -> ~212 ms (live 38), results identical. Since 0029 OCCT's
  own fillet also builds it within band (121 ms), but ~1% R off the exact round: the exact route stays.
- blend-chamfercut (67819a61a, gates on vibe5): `planar_chamfer_cut` (detail.rs chamfer, after sections): equal chamfers
  of straight convex edges between planes that run past their edges' ends are cuts by each chamfer's prism along its
  edge (declined unless every piece taken touches its edge). c2-rings-loft 22 -> 29 of 32 (next: Loft 1, not blend's).
  T1 `features_c::chamfers_running_past_their_edges_ends_cut_the_prism_as_live` (live's 14/36/24, 9761.261 mm³).
- **c2-hunting-knife Fillet 3** (44/108/65, live 38/96/59; volume and area in band): (1) the tip (x ~74): OCCT's round
  pinches to a point at the apex (all four boundary halves end at (74.45, ±1.04, -4.24)), two faces a side; live's
  keeps its width there (its two edges 0.7 mm apart), one face (JjG / JjC 7.8173). Suspect the tip cylinder's seam at
  the apex; WIP blend-knifetip (patch knife-fillet3-analytic-wip.patch: analytic faces accepted by each face's gap x
  area, tori pieces merged) doesn't help as they don't share an edge. (2) The corners near x ~3.85: live's vertex
  patch starts 0.68 mm further along the bottom edge (its line 0.906 mm, ours 1.583) and is one face where OCCT's is
  two: capture asked of live (REQUESTS 19:55). 
- learning-center-michael Fillet 2 parked (lead 11-12): branch blend-michael b24981eeb, patch michael-direct-ball-wip2.
- Hosts: vibe4 gone; vibe5 one job at a time, CARGO_BUILD_JOBS=6, dev profile (release LTO builds got SIGTERM'd).

## 11-12 08:03 UTC: rings-loft speed landed; FACEBLEND-HOLD-RIM built directly (c2-variable-fillet-faceblend 7 -> 8 of 8)
- Landed: blend-ringtips (7106535b), blend-ringspeed (11f138b5: a ring's rounds built directly, Fillet 1 ~423 -> ~97 ms;
  the notch arcs' off-part check by a ray, Chamfer 1 ~244 -> ~183 ms; live 23 / 30). The rest of Chamfer 1 is noshape's
  Boolean with the four lofted tools (~87 ms) and the wedges' splits (~55 ms): the exact chamfer build would be days.
- **FACEBLEND-HOLD-RIM** (blend-holdrim, queued de3ee507; model suite 829 -> 830, 77 -> 78 documents, assemblies identical, g++-13): the held range in pieces, one per chain edge and hold
  line edge (`held_pieces`, as live's 8 faces), each an open loft (24 sections a piece: the radius bends only at the
  pieces' ends), all handed to `k::blend_direct`, which now: takes a lofted side that only approximates its part faces
  as lying on them (`lies_on`: its samples over a face within 1e-4 x size, normals within 0.02 rad; the touch curve
  cut only where it runs near that face), leaves out the planar ends where two tools meet and their blends' shared
  section, runs a shallow run-out section on to the blend's boundary, treats a short tangent zone as a crossing (not an
  overlap), drops a cut within snap of its face's boundary all along (a lofted hold line a few µm off the rim), checks
  OCCT's IN against a tool with rays (OCCT put a family of points inside a lofted tool: its ray through an edge), and
  sews / snaps twice as loose when a side is approximate. Result 20 faces, 136,948.64 mm³ (live 136,943.82). T1
  `features_a::a_face_blend_held_by_a_pockets_rim_takes_the_whole_top` (a stadium plate with a concentric pocket:
  Pappus volume, 14 faces). Face blend 1 ~366 ms (live 25): the 8 lofts ~205 ms, the direct build ~56 ms.
- learning-center-michael's Fillet 2 (603 ms, live 46): 530 ms is OCCT's cut by its 6 tori (the two holes' rim rounds,
  top and bottom) and 6 mitre balls (where Fillet 1's end rounds meet, same radius 14.485). OCCT's own fillet still
  fails it with 0029 ("Unfilleted edge overlapped by fillet.", 1 s). Glued cut 169 ms but wrong (40 faces, live 44);
  tools fused first 2.3 s and wrong. Through blend_direct (branch blend-michael b24981eeb, patch
  /root/blend-scratch/patches/michael-direct-ball-wip2.patch, switches ZZ_DIRECT_BALL / ZZ_SEED_VERTICES / ZZ_SP):
  the ball tool as cube ∩ both rounds' cylinders less the sphere (its faces then lie on the two rounds; the plain cube's
  lay on planes the rounded corner no longer has), the part's vertices seeded (a cut ending at a face's corner closes a
  piece), a vertex shared by two cuts takes in both curves' ends, an intersection curve on a torus that won't
  SameParameter re-projected, a sliver piece with no inside point left out: every face now splits, but sewing leaves
  free edges where the holes' rim rounds run out onto the end rounds at a shallow angle and into the mitre balls (four
  symmetric places) and along the top line rounds. Next: those three-blend meetings (torus x end round x sphere).

## 11-11 23:23 UTC: rings-loft's pattern tips done without crates/brep (17 -> 19 of 32)
- Landed: blend-lcm, blend-filletspeed, the knife's sliver end (af254976; knife 15 -> 16 of 52).
- **Circular pattern 2 as live 2513** (blend-ringtips, queued 1b11b8f6): a feature pattern of an Add and the chamfer or fillet of
  what it made (every reference on the Add's own faces) runs the blend on each copy's New body alone (pattern.rs
  `reapply_as_tools`: the blend after its Add in one scratch, its references moved onto the copy), then adds the
  chamfered bosses in one Boolean. No blend through a neighbour's tip is needed: the tips are a union. The pattern's
  8 tools went to OCCT's glued fuse (noshape's declines "a face lost"), which merged nothing (ring + 8 bosses apart,
  96 faces) and passed the volume check (apart, the volumes add up): `volumes_add_up` now also wants the solids a glue
  left apart not to touch (boxes, then BRepExtrema), else the fuse runs unglued. Result 103 faces, volume 1140.909137
  (live 1140.909064). T1 `engine::reapplied_chamfered_bosses_that_overlap_are_each_chamfered_alone` (main 2673.333,
  live's rule 2652.0 mm³). Gates: model suite 827 -> 829 (no document worse), the 67 recorded glued fuses of the bool
  corpora identical and no slower (alternated), g++-13 builds; assembly pair identical (24/24). polygon_chamfer (blend-parity WIP)
  and the Split-aware `instance_references` (6bbfc3d13) dropped: not needed.
- Speed: Circular pattern 2 now ~1.2 s (live 201 ms): the glued attempt (~0.4 s) then the plain fuse. noshape's Boolean
  declining "a face lost" on this case is bool's (case: vibe4 ~/tmp/blend/case1, from f33de9517baf6402).
- Next difference: **Extrude 5** (an engraving 0.5 deep across the ring's bottom), 342/213 edges/vertices, live
  335/206: our edges split at the revolved ring's seam (y = 0, z < 0; 4 B-spline and 3 ellipse edges in two pieces
  each); Parasolid has no seams. Not blend's (core's unify / seam placement).

## 11-11 20:44 UTC: rings-loft's pattern tips (lead: exact, in crates/brep, till end of 11-12); RABC parked
- Landed today: blend-cliff (362b57e0, the impeller's Fillet 1), the knife's Chamfer 1 speed (11b4acf2). Queued:
  blend-lcm, blend-filletspeed (both gated: model suite =, assemblies = (asm-infento-subs 108 on both sides, four
  alternating runs: main's 113 earlier was its own noise), g++-13). Knife sliver fix (blend-parity 5052819de: a sectioned
  chamfer's edge under 0.02 mm it made collapsed to one vertex, as live's JZQ; knife 15 -> 16 of 52): model suite
  826 -> 827, assembly pair running (vibe4); the knife then stops at Mirror 1 (unify of a surface of linear extrusion
  and its mirror image: core).
- **c2-rings-loft Circular pattern 2** (feature pattern of Extrude 4 + Chamfer 2, 9 copies over 180°; live 103 faces):
  (a) the copies' chamfer references were lost in the pattern merge's splits: `instance_references` now takes every
  piece of a renamed edge whose faces descend (`descends_from`) from the renamed faces (blend-parity 6bbfc3d13, WIP:
  no result change alone). (b) The copies' diamond bosses overlap their neighbours by ~1.3° at the tips (each ±11.9°,
  22.5° apart): each copy's tip edges end in a micro-ridge above the neighbour's already chamfered tip. noshape's blend
  declines ("a wall shorter than the blend", "a chain end with more than three edges", "another edge of a wall within
  the blend"); OCCT fails. Sections (the constant-angle guard off, 9bb672ba7 WIP): every copy chamfered, area
  1435.20 (live 1435.16), but 1e-4..3e-3 mm² fragments at each tip (145 faces); the sliver merge at 0.1 mm gives live's
  103/254/152 but the area 0.037 under (out of band); the plain tool (corner on the edge) is worse (151). Dumps:
  vibe4 ~/tmp/blend/dumps/chamfer-26 (seed), -28 (first copy), -30.. (later). Next: the exact chamfer through the
  neighbour's chamfered tip in noshape's blend (crates/brep/src/blend.rs, bool's gate), till end of 11-12.
- **RABC parked** (SETBACK-FILL-PATCH): live 2499 (core/q2499/out.json; behaviours "blend (2499)"): RABC is a cubic
  B-spline, 15 x 14 poles; live's own G1 fill re-run on RABC's finished edges gives 34.709 mm², not 34.464: live's
  patch depends on the feature's split-edge parameterisation or the faces round the hole; ours 35.641 is above even
  live's G2 refill. Start there.

## 11-11 15:35 UTC: branches and gates; the speed gaps
- Landed: blend-opchamfer (cb0551bf), blend-equal (011fcc6d). Queued: blend-knifespeed's Chamfer 1 commit (a6ffa0de:
  no OCCT retries for a distance-and-angle / two-distance chamfer with sections to fall back on, lofted section tools
  OCCT's Boolean only; same-base alternation 659 / 659 -> 459 / 484 ms; model and assembly suites = base).
- blend-cliff (worktree blend4, d307c7910 + 465ba03dc on d2c2b71ef): the impeller's Fillet 1 built directly (below);
  sections ~3° apart and one at the tube's seam (the faces' edges must pass through the seam point to sew); the wall,
  tube and cap split together (the cap's rim split alike); the wall evaluated as noshape's NURBS (`Shape::face_nurbs`)
  with Newton feet: Fillet 1 3.4 s -> 0.40 s (live 38 ms). T1 `a_round_over_a_tube_end_rolls_the_ball_over_the_rim_as_live`
  (a 30°-tilted wall, the tube's seam under the round and in the band; every blend point's ball R off the wall and 2R
  off the axis or R off the rim, < 0.02 mm). Model suite 821 = 821; assembly pair running (vibe4).
- blend-lcm (worktree blend7, 1344a2271 on the knife commit): `fillet_by_tools`' sphere / torus tools cut by OCCT's
  Boolean straight away (`occt_boolean`), noshape's declining them after its sections; learning-center-michael's Fillet 2
  654 / 647 -> 583 / 591 ms. The rest is OCCT's cut (314 face pairs); gluing it is 3x faster but wrong (40 faces, live
  44); fusing the tools first gains ~5%. Model suite = base; g++-13 OK; assembly pair running (vibe3).
- blend-filletspeed (worktree blend6, ecc05c0e5 + dbeefc647 on 44ff040aa): blend_direct's blend / part-face sections on
  OCCT's pool, merged in their order (cf-snap-hook's Fillet 6 122 / 160 -> 76 / 77 ms); a full round of a bore or boss
  cuts its revolved tool glued (motor-mount-jack's Fillet 2 67 / 69 -> 25 / 39 ms). Fillet 6 alone: model suite 826 =
  826, g++-13 OK; the pair's model suite (vibe5) and assembly pair (vibe4, after blend-cliff's) running.
- Knife Fillet 2 (254 / 278 ms, live 10): OCCT's failing attempt ~72 ms (its build ~37, the rest pre-checks), the
  sections and their run-on ~70, the reface and Boolean ~105. Classifying before the distance in `reach` and a box
  test before `trimmed_run_on`'s distances made it slower (the classifier runs every step; a B-spline face's precise box
  costs about a distance). Not fixed yet.
- c2-tpd-fillet-variable Fillet 1: ~80% is noshape's Boolean's sections-ahead (bool's code): for bool.
- cf-snap-hook Fillet 5 (109 / 132 ms, live 6) is the next of that document's.
- List: the impeller round's tube remainder can stay two faces split at its old seam (live one face JWG); the T1 case
  merges them (12 faces) but the impeller didn't (21 faces before the cap-split change: re-check).

## 11-11 10:55 UTC: impeller Fillet 1 exact (+2,412.23 mm3 vs live 2,412.73), knife Chamfer 1 speed, 0029 landed
- Landed: OCCT 0029 (704ed18d -> fb83a50e; the lead installed its libTKFillet.a / ChFi3d.hxx into /root/occt and
  /opt/occt). Queued: blend-opchamfer (6642171e; model suite 820 = 820, assembly suite = 6364f3c47's, cf-fix-pcb's
  flag was noise). blend-equal: assembly pair re-running on vibe4 (the vibe3 OOM killed the first base).
- **The impeller's Fillet 1: OCCT's round is the wrong surface, not our input.** Live's JWK grid (core/q2479) has its
  ball centres 10.0000 mm off our wall and 20.0000 mm off our tube's axis: our part matches live where it matters.
  OCCT's round (as is in metres, at the working scale, any `SetParams`) is off the exact rolling ball by up to
  0.11 mm at its rails and 0.28 mm across the arc near the rim (0.64 at the end arcs): +2,767 mm3 on the part, +2,592
  on the tube run on past the rim, against live's +2,412.73. (So OCCT's fillets on free-form walls can be ~1% of R
  off live's in general: a candidate for exact sections elsewhere.)
- Branch blend-cliff (worktree blend4, d307c7910 WIP on d2c2b71ef): `cliff_end_round` builds the whole round
  directly: the balls by angle about the tube's axis, on the tube (centre rt + R off the axis, R off the wall, a
  secant on the axial offset) except over the rim's band, where they touch the rim and the wall (the cliff solver);
  their exact arcs lofted into two faces (JWO over the band, JWK round the rest: 173.398 / 1,364.985 mm2, live
  173.434 / 1,364.999; JWO's box = live's to 0.01 mm); the faces' touch edges imprinted on the part as they are (normal
  projection onto the B-spline wall returns nothing); the wall's and the tube's pieces under the round dropped; sewn
  (1e-7 of the part) into a valid solid: +2,412.232 mm3. Booleans don't do it: rim balls tangent to the wall cut to
  nothing, lofts whose arcs touch the wall / tube are taken for disjoint, and sections in the balls' own planes cross
  where the band's end planes meet (a line through the fill's air); an arc 0.1% larger fuses (+2,402.9) but its edges
  sit 0.45 mm short of live's. Left: the tube's remaining face stays two (split at its old seam where the touch
  curve's halves meet; live one face JWG; unify_same_domain_whole doesn't merge them), the zz probes out, a T1 test,
  speed, the suites; the impeller document pair is running (only-cliff1).
- Knife Chamfer 1 (branch blend-knifespeed, worktree blend5, a0834a6c4): 706 ms was OCCT's attempt 174 ms (its first
  build invalid, then three retries at 1-1e-7..1-1e-5, all invalid), noshape's Boolean declining the lofted tool
  60 ms ("an edge nearly on another argument's face") and OCCT's cut 310 ms (117 edge/face pairs on the blade's
  B-spline faces). Now: a distance-and-angle or two-distance chamfer tries OCCT without retries (`without_blend_retries`)
  and goes to its sections at once; section_blend's lofted tools run OCCT's Boolean only (`with_occt_boolean`).
  T1 alternation 648-707 -> 471-512 ms; model suite = base (no "OCCT's retries build it" with NOSHAPE_BLEND_VERIFY);
  g++-13 builds; assembly pair on vibe3. OCCT fails every chamfer of those two edges (even 0.01 mm equal), fillets
  build: no cheap predictor. The cut itself is the rest (~300 ms): oriented boxes, no-Rust, running the chamfer line
  past the faces don't help.
- RELEASE-BUILD-FILLET-HANG's Fillet 6 "1 missing selection": its missing edge is the one Chamfer 4 chamfers (the same
  query); live's the same (behaviours.md:10592). Nothing to fix.
- Next (lead 08:25 / 08:10): finish the cliff round by 18:00 or write down what blocks it; then the speed gaps:
  learning-center-michael Fillet 2 628/46 ms, cf-snap-hook Fillet 6 119/8, knife Fillet 2 213/10,
  c2-tpd-fillet-variable Fillet 1 87/29, motor-mount-jack Fillet 2 33/3.

## 11-11 05:21 UTC: branches in flight, OCCT 0029, the impeller's cliff round
- Landed: knife Chamfer 1 (12b1d8a6), blend-fastfinish (4dc4f096: the finishing passes' sphere / cylinder scans by
  surface type; hat8's Seat lip chamfer 3.2 -> 1.9 s, Riser feet 5.7 -> 5.2 s, results identical).
- blend-equal (worktree blend, 9b4ff66eb): rims tilted to their plane chamfer as one rolling ball's chord
  (`rim_chamfer_tool`: the wedge split by the closed sheet through the ball's touches, seam off the part); rings-loft
  12 -> 17/32, model suite 820 -> 825, the flagged "slower" documents being re-run alternately (only-equal2.out);
  assembly gate running on vibe3.
- blend-opchamfer (worktree blend3, 5929660a1): CHAMFER-FS-OPCHAMFER done (distance face shared / left, Opposite,
  direction overrides, propagation and its stop, Tangent method); model suite 820 = 820 (cf-fix-pcb's slower flag to
  re-run); assembly run on vibe5.
- OCCT patch 0029 (worktree blendocc, branch blend-occt0029 pushed): a per-face sample count cache in
  ChFi3d::IsTangentFaces while a builder's PerformElement / Compute runs. Own OCCT at vibe4 ~/blend-occt (work tree
  ~/blend-occt-work, its git has the patched source; build script /root/blend-scratch/occt0029/build.sh); never in
  /root/occt or /opt/occt. Blend corpus: identical (1 of 310 cases differs on both, as before), 4.05 -> 3.83 s; patched
  ChFi3d files compile with g++-13; Kernel (macOS, Windows) passed on the branch; model suite pair running
  (occ-pair.sh); hat8 re-time next (probe /root/blend-scratch/probes/zz_hat_probe.rs, fresh mode-700 copies on vibe3).
- Impeller Fillet 1 cliff round (worktree blend4, branch blend-cliff, WIP not committed): live's JWO is the ball
  touching the wall and the tube's end rim (centre in the rim's normal plane, R from the rim point and the wall):
  computed over the dump (vibe5 ~/core-cases/dumpfil2/fillet-9), the cliff range is the rim angles -32.3..+33.2 deg,
  rim ends at z -25.6, wall touches y -7.2 / +7.5, top z -7.609: live's JWO box. `cliff_end_round` builds OCCT's round
  on the part with the tube run on 3R (valid, +3,075 mm3; +2,592 outside the cliff range) but the cliff part's 2D
  sections (rim point, ball arc, wall touch, a point inside) fail: in those tilted planes the wall / tube gap is open,
  no corner or inside point is reliable. Also: OCCT's round outside the cliff range alone adds 2,592 mm3, more than
  live's whole fillet (+2,412.73): the main round itself may differ from live's (JWK 1,365 mm2; ours 1,261 in two
  halves), to compare before the cliff part. Next: the cliff face built directly (the pipe of the rim balls trimmed by
  the wall touch curve and the rim arc, sewn in; blend_direct's way), and the main round's difference first.
  Correction to core's write-up: live's cap JVG is 201.062 mm2 before and after (an annulus round the bore), as ours.

## 11-10 22:38 UTC: knife's Chamfer 1 queued (a11be226); the chamfer rule; the impeller's chamfer is a mitre
- Live 2462 / 2441: a Face offset chamfer is the chord between the feet of a point at fixed offsets from both faces
  (equal distance: one rolling ball), the offsets sized so the asked distances (and angle) hold at one place: the edge's
  middle (2441: R = √3 / 1/√3 for a tilted hole / plane), for a chain the middle of its longest edge (2462: sizing at
  the chain's middle is 0.0017 mm off all along). Where the faces' angle is constant this is OCCT's chamfer.
- Knife Chamfer 1 (`ChamferRef::offsets`, `feet_offsets`): when OCCT fails and the angle varies along the chain, the
  sections keep those offsets; strips within 0.0005 mm of live's 34 samples (0.0019 near the tip); 19/51/33, live
  19/50/32: live has no edge between the strip and the slope at x -7.8, where its strip is still 0.011 mm wide (most
  likely a Parasolid tolerant vertex, 11 µm against its 10 µm blend tolerance); not copied. Frames oriented once per
  edge, 24 sections: 3.1 -> 1.3 s (live 19 ms; the cut and OCCT's failed attempt left). A constant-angle chain stays
  OCCT's (c2-rings-loft's Circular pattern 2 replays Chamfer 2, which OCCT fails on the copies: the fallback made it
  183 faces, live 103). Model suite 818 = 818, assembly suite unchanged. T1 features_c::a_distance_angle_chamfer_
  keeps_constant_offsets_as_live.
- Equal-distance rule (rings-loft), local branch blend-equal (patch /root/blend-scratch/patches/equal-wip-all.patch, not
  built yet): `rim_chamfer_tool` (ring_blend's chamfer: the ball's touch curves exact from its centres, a closed loft
  of triangles, corner in the air), `rims_tilted` (tilted rims to ring_blend even when they don't overflow), T1
  equal_chamfers_on_tilted_rims_roll_one_ball_as_live (live's 97.880 / 52.661 mm²). A whole rim's middle follows
  Parasolid's ellipse start (the major axis end opposite the cylinder's axis seen on the plane): our cylinder's axis
  sign may differ in translated documents.
- The impeller's Chamfer Stevenrohr Halterung (core's hand-off below), on main + the knife change (blend2, vibe3
  /tmp/blend-imp/chamfer-13): the three edges lie between the holder's x = 5 plane and the arc's cylinder / two thin
  extrusions, 90° all along; the arc and each 0.31 mm piece meet at an 8.9° kink (not tangent). A 3.5 mm / 20°
  chamfer (1.27 mm on the other face) ten times wider than the pieces: OCCT fails at the two mitres; live adds three
  faces. Next: build such a chain as each edge's chamfer region (OCCT per edge works) mitred at the kinks.

## From core (2026-11-10 19:36 UTC, lead's list): c2-impeller-loft's Chamfer Stevenrohr Halterung (26) and Fillet Stevenrohr zum Halter (29)
- qa flagged both as regressed (built at 4e549553, ERROR since): they built only while Boolean 1 had failed; core's
  translator fix (region probe along a spline, landing) now gives live's rib: Extrude 4's Part 3 5 faces 1,748.949 mm3
  (live 1,748.96), Boolean 1 OK (29 faces 71,830 mm3; live 30 / 71,495, the Shell / Fillet 1 difference upstream).
- The chamfer (OFFSET_ANGLE 3.5 mm, 20°, tangent propagation) takes three edges of the holder's x = 5 face: the 53.6 mm
  arc (edge 20) and two 0.312 mm B-spline pieces (21, 22) at its ends, where each rib spline dips inside the arc's
  circle and crosses it again (the sketch's 0.0025 mm2 slivers; live's sketch has them too). Each edge alone chamfers
  (71,566.102 / 71,837.800 / 71,837.800 mm3); all three: "chamfer failed". The fillet (R2, no tangent propagation,
  4 edges) now says "Adjoining edge not filleted", likely the same junction. Live builds both (33 faces 71,375.52;
  39 faces 71,063.84 mm3).
- Dump vibe4 ~/core-cases/dumpimp/chamfer-21.brep (+ .txt: edges 20 21 22, faces 13 12 6), probe
  /root/core-scratch/hb/bool_patterns-impchamfer.rs (`probe_imp_chamfer`). Yours (blends); core moves on.

## 11-10 11:35 UTC: knife's Chamfer 1 builds as sections, waits for live (local branch blend-chamfer, WIP commit)
- Fillet 2's first land failed (kernel_robustness::oversize_fillets_like_onshape: the overflow fallback also took
  convex lines and built rounds live refuses); now concave only, re-queued as 472c67a7.
- Chamfer 1: OCCT's distance-angle chamfer fails along the blade's cutting edge (any size, either reference face,
  each edge alone). WIP fallback (`BlendSpec::reference`, `ChamferRef`): when OCCT's chamfer fails, sections along
  the chain, `r1` on the reference faces, the other distance from the angle and the faces' angle at each section
  (law of sines), each face's direction the chord to its point at `r1` (a hollow grind), the section's corner out
  in the air (the tool's sides needn't lie on the B-spline), one loft per edge (live: one face each), free chain ends
  run on 2 x the size. Knife: Chamfer 1 19/51/33 (live 19/50/32); the curved strip left 0.055 mm wide (live 0.062),
  the straight one 0.060 (live 0.022): the angle's reading is off on the straight part. Asked live (REQUESTS blend
  11:35) for JZC / JZG's definitions and the edges round them. Patch /root/blend-scratch/patches/chamfer-as-sections-wip.patch,
  probe features_c zz_probe_knife_chamfer1 (ZZ_KNIFE = vibe5 /tmp/blend-knife-c1.brep, Chamfer 1's input).
- With the WIP, past Chamfer 1 (report.json's differing list, 12:45): Mirror 1 23/64/42 (live 22/60/39: our extra
  edge doubled, and a face live joins across the mirror plane), Fillet 3 45 faces (live 38, blend's, partly
  inherited), from Curve pattern 1 the volume -3.8 mm³; later errors are queries / translation (Fillet 5 / 7 no
  entities, Loft 1 profiles, Extrude 8 / 9 / Revolve 1 no merge scope, Split 4 / Extrude 10 no faces). Chamfer 1
  takes 1.7 s (live 19 ms): the per-section projections and OCCT's failed attempt first; to speed once its shape is
  live's.
- Fillet 2's land: 2nd and 3rd attempts failed on core's OCCT lib sync (the STEP read test 0.5 -> 6.8 s, and a rustc
  crash); landed 13:32 as 76cbef0e.
- The impeller's "Chamfer Stevenrohr Halterung" / "Fillet Stevenrohr zum Halter" failures (qa's 4f5863a2 run): upstream,
  handed to core (core.md 11:01).
- Asked live 07:58 for the equal-distance chamfer rule (tilted hole, cylinder under a tilted plane): rings-loft waits.

## 11-10 10:29 UTC: knife's Fillet 2 as live (landed 13:32 as 76cbef0e), Chamfer 1 next; fillet fallback costs
- Fillet 2: `BlendSpec::overflow` (a fallback after OCCT's fillet fails, overflow on, all lines): a concave line's
  section runs on 2R past its free ends; the faces its run-on pieces touch (supports aside) trim it, planes extended
  by `k::extend_face`, a B-spline by the new `k::extend_face_over` (only the sides the run-on's points over the
  body lie beyond, by what they need: a long all-sides extension of the 4 x 6 grind spline broke OCCT's
  intersections). The body's face is rebuilt on that extension first (`k::reface_on`), so the round's piece on it
  merges in the unify; `k::join_edges_at` then joins the old line edge and the straight section running on from it
  (OCCT's unify won't join a line and an approximated B-spline). 17/46/30, +0.3954 mm³ (live 17/46/30, +0.3955),
  grind face to x -3.1588 (live -3.1588). T1 features_c::a_round_running_off_its_edge_meets_the_grind_face_
  extended_as_live (data/knife-fillet2-input.brep). Model suite 816 -> 817 (only the knife, 14 -> 15/52),
  assembly suite unchanged (4/24, 1261/1731, 170/345), g++-13 clean.
- Knife's next difference, feature 15 Chamfer 1 (offset 0.45 mm, angle 22.5°, opposite on, the two edges where the
  grind face meets the cutting edge's strips: a B-spline along the curved part and a line along the straight part,
  tangent at (-49.99, -0.25, -19.13)): OCCT's distance-angle chamfer fails on them together, each alone, either
  reference face, and on the body before Fillet 2 too (not the rebuilt grind face). Live: two faces, 35.79 and
  22.13 mm². Probes /root/blend-scratch/patches/knife-chamfer-probes.patch (dump vibe4 NOSHAPE_DUMP_OPS chamfer-5).
- Item 5 (fillet fallbacks, kernel profile of the whole model suite, vibe4, /root/blend-scratch/prof-suite.log,
  bin/fallbacks.py): 8 blends per regeneration take a fallback after OCCT's whole set fails; the failed as-is +
  scaled attempts cost ~1.9 s in all, 1.5 s of it the impeller's Fillet Ansaugflansch zum Gehaeuse (as is 70 +
  scaled 630-700 ms) and Fillet Bohrungssitz Ansaugflasch (75 + 480), whose by_groups then costs 1.5-2.0 s more;
  the rest: Fillet 4 / Fillet Stevenrohr zum Halter (impeller, 160-240 ms failed), the knife's Fillet 2 (73), two
  Fillet 1 / Fillet 2 of 50-90. by_groups rounds each vertex-connected group alone and takes their common: OCCT
  fails the set where separate groups' rounds meet. Candidate test: two groups whose edges' boxes, grown by their
  radii, overlap → groups first (and the scaled retry skipped).

## c2-hunting-knife's Fillet 2 (11-10 07:56 UTC: superseded by the 10:29 entry)
- The edge: a 0.95 mm concave line (x -4, z -14.43, y -0.95..0) between the floor and a 55° slope, R 3, overflow
  on. Its y -0.95 end is where the blade's grind face (a 4 x 6 B-spline) cuts in. Live: the round runs straight on
  past that end across the whole half-blade (y -2..0), trimmed by the grind face extended below the floor (the
  grind face grows 0.466 mm², the side y = -2 gets a 0.023 mm² sliver): 17/46/30, +0.396 mm³, one round face (JYC).
- Ours: OCCT's fillet builds the same cylinder (x -4.896..-2.438) but puts a 0.82 mm² B-spline patch where the grind
  face should extend, and the solid is invalid → "Failed to fillet selections.". The section route (propagation
  off) stops the round at the edge's end with a cap: 19 faces, +0.348.
- Prototype (`BlendSpec::overflow`, `trimmed_run_on` in blend_section, a fallback after OCCT fails): the concave
  line's section runs on 2R past both ends; the run-on pieces' neighbouring faces (grind, side y = -2, mid plane
  y = 0) extended split the tool, the pieces inside all of them fused. With the grind face extended 0.9 mm: valid,
  +0.390 mm³, 18 faces (the extension stays its own face). Fragile: at 0.6 mm the fuse is invalid, from 1.2 mm on
  the extended B-spline no longer splits the tool (a kernel probe: OCCT's section of a box with that face extended
  6-12 mm gives 1 edge, and the split loses volume). Probes: features_c zz_probe_knife (ZZ_KNIFE = dump), kernel
  blend zz_knife_* (ZZ_CASE = vibe5 /tmp/blend-knife/fillet-3).
- What a real fix needs: extend only the face's sides the run-on crosses, by the length it needs (not all four by
  2R), and merge the extension into the face (`k::merge_faces_on` on the extended surface) for live's 17 faces.

## From core (2026-11-09 14:22 UTC, covering blends): c2-impeller-loft's Fillet 1 needs a cliff-edge round
- With core's Delete face 1 (c7c48f81) the impeller holds through feature 20; feature 21 Fillet 1 (R10, one closed
  edge where the stern tube's cylinder JSO meets the housing's outer wall, a B-spline; Allow edge overflow OFF)
  differs: ours +2,767.16 mm³ and 3 round faces, the tube's x 120 end cap cut to 201.06 mm²; live +2,412.73 mm³,
  JWK (OTHER, 1365.0 mm²) + JWO (OTHER, 173.43 mm², x 113.8..120, z -25.6..-7.6), the cap JRK untouched (314.159).
- Live's edges there: JWJ, an 11.5 mm arc of the cap's rim (120, 0.06, -24); JWV / JWZ, two 20.38 mm circles at
  (114.76, ±2.4, -17.97) (the ball's sections where the round becomes the cliff part); JWR, 19.57 mm on the wall.
  So where the round's contact on the tube would run past its end, Parasolid's ball rolls over the rim edge (the
  cylinder's end circle) instead: JWO is the ball's sweep touching the wall and that edge, between the two sections.
- Ours: blend.as_is / blend.scaled fail, blend.fallback.split_closed builds OCCT's round trimmed by the cap.
  detail.rs's cliff routes cover a planar face narrower than the ball (section_blend cliff) and boss rims
  (cliff_rim_blend), not a round running off a cylinder's end against a free-form wall.
- Repro: dump vibe5 ~/core-cases/dumpfil2/fillet-9 (+ .txt: edge 10, R 0.01, checked); a kernel probe must call
  set_working_scale(1000) as the model does (probe_fillet_dump in /root/core-scratch/imp/features-with-fillet-probe.rs).
- Downstream: feature 24 Extrude 4 then fails ("up-to target does not fully bound the extruded profile", core's
  to recheck once this round is live's).

## From core (2026-11-09 13:12 UTC, lead: core covers blends while blend is paused): the impeller's three slow fillets
- Fillet Flansch zum Stroemungskanal 2.2 s (live 67 ms), Fillet Ansaugflansch zum Gehaeuse 1.6 s (321), Fillet
  Bohrungssitz Ansaugflasch 1.6 s (246) (kernel profile, vibe5, /root/core-scratch/fil.log): each fails OCCT as is
  (blend.as_is) and scaled (blend.scaled, ~0.4-0.9 s), then builds by groups (blend.fallback.by_groups ~1 s).
- Not worth speeding as they are: they run on a part that is no longer live's. The impeller's chain breaks at
  Delete face 1 (feature 18: our OCCT defeaturing gives the part back unchanged, live removes 23,456 mm³), with
  Boolean Flansch+Gehäuse (feature 15) already 15 faces for live's 19; by Fillet 4 live has 64 faces, ours 34, and
  Fillet Flansch zum Stroemungskanal resolves 4 of its 18 edges. Nothing changed in blend's code. Dumps: vibe5
  ~/core-cases/dumpfil (NOSHAPE_DUMP_OPS), probe `probe_fillet_dump` in /root/core-scratch/imp/features-with-fillet-probe.rs.
- For blend, after the chain matches (lead 11-09): the fillets' speed then. The route "whole set fails (as is, then
  scaled), then group by group" costs ~0.4-1.3 s of failed attempts per fillet here; a cheap test that predicts the
  whole-set failure (what makes OCCT fail on the set but not on its groups) would let it go to the groups first.

## From dx (2026-11-09 11:40 UTC, lead: blend paused till Monday): c2-fan-blade-sweep's Chamfer 2 fixed
- Cause: OCCT's chamfer kept the thread's band (91 faces, core's keep mark on its edge), then `merge_full_rounds`
  (finish_blend_faces) joined the band and the boss next to it: two coaxial cylinders of one radius whose other edges
  make one loop, its rule for a full round's halves. It now leaves a kept edge alone. And the fillet / variable fillet
  / chamfer copies of their input (`BRepBuilderAPI_Copy`) dropped every keep mark: carried now (`carry_kept_marks`),
  as the Booleans' copies do. crates/brep untouched (no gate needed). T1 blend::a_chamfer_keeps_a_kept_split_of_a_cylinder
  (fails without it).
- Source studio fb207f79: 18 -> 22/22, to the end. With dx's two other changes (Derived 2's qEverything(BODY) in the
  parity translator; the STEP blade's last edge joined in the import) c2-fan-blade-sweep 0 -> 23/53; next: feature 5
  Extrude 1 ("klammer" 7/15/10, live 6/12/8; its Sketch 2 has untranslated entities: a zero-length line, 12
  PROJECTED not found), dx's to look at.
- Checks: model suite 75/87, 764 -> 787/951 (only c2-fan-blade-sweep moved), kernel 351 tests, model features_a,
  exchange suite unchanged (46/48 ours imports live's), g++-13 clean.
- Seen on the way, for core: a thread's band on a full cylinder doesn't survive a REMOVE extrude that cuts the boss
  in half (thread M8 on a Ø8 x 20 cylinder, then a rectangle on Top x < 0 extruded 25 mm REMOVE: one Ø8 cylinder
  face left of two). Not needed by any suite document so far.

## From core (2026-11-09 02:48 UTC): c2-hunting-knife now stops at Fillet 2 (feature 14)
- With core's guided loft fix (Loft 3 as live's face, the blade 4983.62 mm³ in live's band), the knife runs 11 -> 14/52.
  Next difference: feature 14 "Fillet 2": ours ERROR "Failed to fillet selections.", live OK; "4.5\" BLADE" 16/43/28
  faces/edges/vertices before it, live 17/46/30 after. Run it alone: NOSHAPE_PARITY_ONLY=c2-hunting-knife parity_models
  (38 features after it): yours.

## From core (2026-11-09 00:54 UTC, lead's call): c2-fan-blade-sweep's source studio now stops at Chamfer 2
- With core's keep mark (intentional splits survive later unifies), the source studio
  (NOSHAPE_PARITY_MODELS=$HOME/parity/models/c2-fan-blade-sweep/derived NOSHAPE_PARITY_ONLY=fb207f797bca6b31509dba54)
  matches through Extrude 7 and Extrude 8: 18/22. Next difference: Chamfer 2, 90 vs live's 91 faces on "Poti2":
  our chamfer drops the 0.15 mm band External thread 1's Split face made on the boss (edge circle at x -13.8, r 3.4;
  marked "keep" since this change: `kept_edge()` in common.cpp, carried through copies with `carry_kept_marks`). That
  one face then carries into Derived 1 (82 vs 83), so the document stays 0/53 on it: yours now.

## From core (2026-11-08 20:05 UTC, lead's suite ranking): c2-fan-blade-sweep's 53 lost features start at a chamfer
- c2-fan-blade-sweep (0/53) fails at Derived 1 because its source studio fb207f797bca6b31509dba54 fails at feature 8
  "Chamfer 1": ERROR "Failed to chamfer selections.", live OK (Fan-Connector 32/78/52 faces/edges/vertices, live
  48/110/68). Run it alone: NOSHAPE_PARITY_MODELS=$HOME/parity/models/c2-fan-blade-sweep/derived
  NOSHAPE_PARITY_ONLY=fb207f797bca6b31509dba54. The biggest cascade in the suite: yours.
- Also yours now (lead 20:07): c2-rings-loft (12/32, 20 lost) fails at Circular pattern 1 on area alone: "Extruded
  ring" 1192.40 mm², live 1193.07 (-0.056 %), with live's counts (273/762/508); the shortfall is the ring's blend
  faces (core 11-07 19:56, core.md "seam pieces shown as one edge").

## Feature audit (lead 11-08 ~11:00): Fillet / Chamfer / Face blend options, tickets in TICKETS.md "Blends (blend)"
- Landed 11-08: 823698ce (face blend empty options, partial plane end, drafted-rib full round, CHAMFER-METHOD + DA on
  a round face (`blend_section::chamfer_offsets`), FACEBLEND-HOLD-TANGENT (`Sides.hold`, `hold_ranges`), stopped
  chamfer setbacks, face blend Adjacent / Custom (`grow_side`), Detached, Long / No trim error); 660f7ddd (tapered-rib
  cone `tapered_round_tool`, chamfer angle >= 90°, FACEBLEND-SURFACES `sheet_blend` (live 2321's alignment rule),
  detached No trim `run_on_round`); 66bf6d57 (blend_direct skips sections of rounds tangent along a shared circle:
  cf-snap-hook Fillet 3 91 -> 54 ms). Queued 116f47c6: variable fillet warnings (live 2325 A) and constant case.
- Every change ran the before/after suite on vibe5 (audit2..10); flagged "slower" documents re-run alternately
  were noise each time.
- opChamfer: routing it through the Chamfer evaluator broke cf-snap-hook; "Opposite off = the edge's second face"
  (live 2325 D) broke cf-spur-gear: both reverted; asked live 16:34 for the rule (caps / sides / version).
- Parked: FACEBLEND-HOLD-RIM (c2-variable-fillet-faceblend's Face blend 1, the only blend first-difference in the
  suite), branch `hold-rim-wip` (patch /root/blend-scratch/patches/hold-rim-wip.patch, ZZ_HOLD_RIM / ZZ_ONE_BY_ONE
  debug env). Radii match live (13.858 at (50, 25): live's JPC down to z 1.14) and the tool removes 5897 mm³ (live
  5898.7). One closed loft: 5 solids. Pieces split where another hold line takes over (5 lofts) + sections reaching
  0.1 r past the hold line into the pocket (`rim_section_face`): valid, main solid 136951.04 (live 136943.82) but 32
  faces and two stray solids; cut one by one, piece 3 (round the y 25..75 end, r up to 13.9) breaks it: its loft's
  box is ±230 mm. Next: that piece's sections (twist / a jump in r at the chain's vertex at (50, 25)).
  Tried 15:10: sections all started on side 1 (the frame swapped where the edge lists the wall first): the lofts
  come out invalid (7 faces each), still 3 solids (patch hold-rim-wip2.patch). Stopped there (time box).
  17:10 (branch hold-rim-wip3, patch hold-rim-wip3.patch): piece 3's sections are smooth (r 14.8 -> 2.5 -> 14.8,
  frame consistent; the ±230 mm box was the loose B-spline box). New cause: the tool's sides lie on the walls, and a
  loft's side only approximates the curved, filleted end walls. Lifting both sides off the part (`lifted_section_face`,
  into air for a cut) gives one valid part with the closed loft (2233's T1 still passes), but 32 faces (live 20) and
  only 4,858 mm³ cut (live 5,899): the rim top (2,859 mm²) stays though it lies inside the tool. With one loft per hold
  line the pieces' end sections touch: 2 invalid solids. No live capture needed: our radii and the tool's volume match.
  18:40 direct-build route (branch held-direct-wip, patch hold-rim-direct.patch: held lofts handed to
  k::blend_direct before the cut; blend_direct keeps a face whole whose cuts all lie on its boundary): the plain loft's
  wall side, a B-spline only near the curved end wall (a cylinder), gives 18 spurious sections with it: "a part face
  that wouldn't split"; the lifted loft has no face on the part: "a tool with no blend face or none on the part".
  Next idea: a hold-line blend at the rim's 90° is a rolling ball whose radius is the hold line's distance, r(t):
  OCCT's variable fillet over the whole tangent loop (one law per contour: BRepFilletAPI_MakeFillet::SetRadius(law,
  contour)) builds it with its tangency natively; our kernel has single-edge laws only (fillet_variable_checked).
  19:30 tried it (branch hold-rim-varlaw, patch hold-rim-varlaw.patch: op_fillet_variable_points places (point,
  radius) samples on OCCT's contour by RelativeAbscissa): OCCT fails at the exact radii (and at 1 - 1e-5); at 0.999
  it builds one valid part, 23 faces, 140,239 mm³ (live 136,944): a ball rolling with a fast-changing radius touches
  elsewhere than r from the edge, so it takes ~2,600 mm³ where live's circular sections take 5,899 (our loft tool:
  5,897). So the section loft is right; what's missing is placing it.
  What would fit: in blend_direct, a tool face that only approximates its part face (a loft's side on a curved wall)
  treated as lying on it: the tool's touch edge (t1 / t2 curves) projected onto that part face as the cut there,
  instead of sectioning the two surfaces (which gives the 18 spurious curves). A day's work, not an hour's.
- Asked live 12:50: surface walls' side directions (2306 D's apart case contradicts the touching rule).

## Cascades (lead 11-08 ~20:30)
- c2-fan-blade-sweep: Derived 1's source (derived/fb207f797…) failed at Chamfer 1 (two distances 0.4 / 0.2 on four
  0.7 mm pins' tops): our Distance 1 face was the top on three pins (the feature-wide first side). Fixed (b3f3df84e,
  `face_order_in`: each profile loop's first side): the source 8 -> 14/22; Fan-Connector as live. The document still
  stops at Derived 1 on part "Poti2" (82 vs 83 faces), from the source's Extrude 7 (feature 15): core's.
  Run the source alone: models dir with a link to it (vibe5 /home/ngyekta/blend-scratch/models/fan-source).
- c2-rings-loft: the 0.06 % area at Circular pattern 1 starts at Chamfer 1: ours 705.288 mm³ / 914.52 mm² against live
  705.853 / 914.99 (inside its band; Shell / Extrude 3 / the pattern carry it, the pattern's narrower band fails).
  Faces: our chamfer faces 10.044 mm² (live 9.8228), y to 1.81 (live 1.89); the side strips 8.854 (live 9.17, ours
  cut to y ±2.49, live ±2.53): the run-on past the arc's ends goes further than live's. Asked live (REQUESTS 21:06)
  for the chamfer face's surface and edges.
  Live answered (2384: blend/q2384/chamfer-run-on.json): JRC is one ruled B-spline (degree 1 x 3, 9.8524 mm²) running
  smoothly past the arc's end, trimmed by the strip plane JRy along KRpB to the strip's ellipse edge at y ±2.5349 (ours
  ±2.49). So live's run-on is its chamfer surface continued, ours the rim's chamfer continued along the rim. Not built
  yet (quota): next, compare KRpB's 17 points with our run-on's edge, then build the run-on as the chamfer surface
  extended along its own ruling (blend_section ring_blend RimBlend::Chamfer).
  11-10 07:15 (scripts /root/blend-scratch/runon/*.py, our dump ours.txt): the run-on is not the difference. Live's
  JRC rows lie exactly on the notch cylinder (r 3.13739) and the side plane, and every ruling is the chord of one
  rolling ball, R = 0.9196 (all 9 grid rulings within 2 µm, past the arc's end too): the setback is 1.000 only at the
  arc's middle (-62°), 0.917 at its top end, 1.011 at the notch's bottom (dihedral 85-95° along the arc), the same on
  both faces. R = d tan(φ_mid / 2), φ_mid the dihedral at the edge's middle. Ours (OCCT's chamfer) keeps 1.000
  everywhere, hence the strip cut to ±2.49 (live ±2.53). Live carries that ball round the rim past the arc's end, as
  ours carries the rim's chamfer. Fix in ring_blend: the wedge's rim filleted at R, each round face replaced by the
  ruled face between its two contact edges. Asked the lead whether live's chamfers in general follow this rule
  (it changes every equal-distance chamfer on an edge whose dihedral varies).
- opChamfer distance-and-angle side: live 2336's rule (off = cap) breaks cf-spur-gear's gear 8; reverted, see
  TICKETS CHAMFER-FS-OPCHAMFER (patch opchamfer-cap-rule.patch).

## Speed survey (lead 11-08 19:40: audit11's blend features slower than live, by ms over live)
Ours over live, from each document's slowest features and live.jsonl's per_feature_ms (only documents matching up to
the feature count):
- learning-center-michael Fillet 2 640 / 46: fillet_by_tools' need_ball cut, OCCT 314 face-face pairs: bool's (bool.md
  "From blend 11-07 23:10"); blend_direct can't take its ball / concave-corner tools (their faces border other
  tools, not the part: "a tool with no blend face or none on the part").
- c2-rings-loft Fillet 1 392 / 23, Chamfer 1 345 / 30: the same kind (ring_blend's cut), bool's.
- cf-part-draft Part Draft 2 368 / 39: op_defeature (OCCT defeaturing) 254 ms inside the custom feature, not a blend.
- c2-hunting-knife Chamfer 1 182 / 19, Fillet 3: after its first difference (Loft 3): not comparable.
- c2-tpd-fillet-variable Fillet 1 83 / 29: the Rust Boolean's one-by-one 35 ms + sections_ahead 46 ms, bool's.
- cf-snap-hook Fillet 3 54 / 7 (66bf6d57: was 91), Fillet 6 121 / 8 (blend_direct sections, cone x torus pairs
  tangent at the chain's joints), Fillet 5 118 / 6 (Rust Boolean, bool's).
- motor-mount-jack Fillet 2 39 / 3: the full round's tool now one revolve (4d21d6419); the cut itself ~35-45 ms.

## From core (11-07 19:56 UTC): 2e49088a9 regresses learning-center-michael and cf-lighten
- Corpus on vibe5, my tree rebased onto main: learning-center-michael 7 -> 5 (Fillet 2 now fails, "Failed to fillet
  selections.", after 22-52 s; live OK in 46 ms) and cf-lighten 5 -> 4 (Lighten 1's area 48549.85 mm², live
  48550.21). cf-surface-pattern 34 -> 36 in the same run.
- With 2e49088a9 undone in my tree (and nothing else changed), learning-center-michael matches to the end again
  (Fillet 2 662 ms). Repro: `NOSHAPE_PARITY_ONLY=learning-center-michael,cf-lighten` parity_models.

## List (lead 11-07 18:00)
1. cf-surface-pattern's Fillet 1 (side rounds against the slanted top extended).
2. c2-tpd-fillet-variable's variable Fillet 1: exact from its radius law, and fast (1.7 s, live 44 ms).
3. RING-BLEND-SPEED: c2-rings-loft's Fillet 1 370-410 ms vs live 23 ms (OCCT's cut of the two rounds).
4. Face blend tangent hold lines, once live answers REQUESTS core 17:27.
5. Blend differences in the parity suites / SCORE.md, most features first (cf-snap-hook's Fillet 4 is skasm's).

## State

**11-07 18:50 UTC: cf-surface-pattern's Fillet 1 as live (34 -> 36 of 39).**
- Live 2227: the round is the ellipse's alone (JtS top x nose cylinder JnW); a picked face's edges aren't propagated
  (live 2202: "with a face picked, the result doesn't depend on propagation"). Past each end of the ellipse the round
  runs straight on between the side plane (the cylinder's tangent plane there) and the top's plane (cylinders R 2.54,
  JuW / Jua) until the sweep below trims it, 1.617 mm up the side edge.
- `blend_section::run_on_rim_blend`: for a selection that stops where its plane x cylinder arcs run on tangentially
  into edges left out: a wedge (the slab under the plane with the arc and its two end tangent planes), its three top
  edges rounded by OCCT, the wedge minus its round cut from the body. G7's rule ("Adjoining edge not filleted." for a
  curved edge left open) now only where the tangent neighbour shares one of the edge's faces (G7's arc and lines share
  the top; Part 5's ellipse and side edges share none).
- 10/22/14, live's. T1 model features_c::a_rounded_arc_runs_straight_on_past_its_ends_as_live (Part 5 as imported,
  data/surface-pattern-part5.brep: face picked with propagation, and the edge alone without).
- The document's time isn't the fillet's (44 ms, live 11): Surface Pattern 3 7.7 s (live 537 ms), the next difference
  (11 parts, live 6): custom feature, not a blend.

**11-07 20:20 UTC: two P0 regressions (lead), both fixed in one change.**
- cf-snap-hook's Fillet 5 (85 faces into Shell 1, live 30; Shell ~27 s): not core's run-on change. skasm's 5baedbe49
  made Fillet 4 live's (each boss rim a torus R5 from its cap to the floor); Fillet 5 (R1, 4 concave circles where
  those tori meet the floor) then went through OCCT's fillet, whose volume is live's (401925.854 vs 401925.861 mm³)
  but whose round is 16 approximate B-spline pieces an edge, and Shell 1 crawled over them. `coaxial_round`: a whole
  circle between faces of revolution about it (plane across the axis, coaxial cylinder / cone / torus / sphere, a
  torus or sphere among them) rounds as a torus: the 2D round of the two faces' sections revolved, the region added
  (concave) or cut (convex). Its fuse runs at the working scale with Glue Shift (new `BooleanOptions::scaled`):
  306 ms in metres, 38 ms so; Fillet 5 68 ms (live 6; frame / containment checks ~7 ms an edge left), Shell 1
  535 ms (it was 5.8 s before Fillet 4 changed). Snap-hook 9 -> 11 of 18; next: Plane 1 "Point-Plane requires a
  point and a plane." (not a blend). T1 model features_c::rounds_where_boss_tori_meet_the_floor_are_tori_as_live
  (data/snap-hook-case.brep: the case as Fillet 4 leaves it).
- learning-center-michael's Fillet 2 (failed after 22-52 s, 7 -> 5): my 2e49088a9's "a picked face's edges aren't
  propagated". The translator gives Fillet 2's eight edge queries as seven edges and the cap face (one query maps to
  a Face), so the face's edges stopped propagating, the selection "stopped" and went to the section route. Now only a
  selection of faces alone isn't propagated (live 2202's case); with edges picked too, all are, as before.

**11-07 21:40 UTC: c2-tpd-fillet-variable's Fillet 1 156 -> 79 ms (live 29), still to the end.**
- core's exact round (44368ecdd) ran after OCCT's variable fillet failed (~70 ms). `variable_round_overflows`: a
  round wider than a face beside it (the law's largest Bézier value x tan(half the normals' angle) past the face's
  far side) goes to `variable_round` first, OCCT's only if that fails.
- Left: the cut, ~70 ms in noshape's Boolean, half of it a one-at-a-time attempt that fails (the rounds meet at the
  corners): asked bool (bool.md "From blend 21:34").
- Suite before / after on vibe5: 694 = 694 features; the four "slower" documents have no variable fillet (Mirror /
  Extrude / pattern times: load).

**11-07 22:15 UTC: RING-BLEND-SPEED, what was tried (nothing landed).** c2-rings-loft's Fillet 1 ~340 ms (vibe5):
the tools (two wedges, OCCT's lone-rim fillets, wedge minus round) ~25 ms, the final cut ~307 ms in OCCT
(boolean.build; 212 ms of it "Edge-Face intersection", 152 calls: the rounds' B-spline contact edges lying on the
plane and the bore, and the rims on the tools' strips). Tried:
- The rims are mirror images about x = 4 (square to the bore's axis), their rounds meeting on that plane: each region
  trimmed to its side (`mirror_pair`) so no blend-blend meeting curve is left to find. Same topology and volume, but the
  cut stays ~309 ms (and the trims cost 60): the time isn't the meeting curve, it's the edges lying on the faces.
- The cut at the working scale (`BooleanOptions::scaled`): 307 -> 156 ms, but Shell 1 then fails after 25 s on its
  result (12 -> 9 features). Not usable as is; fuzzy 1e-7 x size changes nothing.
- Next levers, in order of size: (a) the tool built with exact contact curves (both are ellipses: the spine, an
  ellipse where the offset plane meets the offset cylinder, projected onto the plane and radially onto the cylinder),
  only the blend face approximated: OCCT's edge-face work on exact conics is cheap; (b) no Boolean: the rims' faces
  re-bounded by the contact curves and the blend faces inserted, the two rounds' meeting curve and run-outs computed
  from their geometry (the lead's "built directly", crates/brep, gated). (a) is a day, (b) several.

**11-07 22:30 UTC: RING-BLEND-SPEED parked (lead: poor value for one 340 ms fillet while quota-limited).** Numbers and
the two designs ((a) exact contact ellipses in the tool, ~1 day, Fillet 1 to ~130 ms; (b) no Boolean, the rounds,
their meeting curve and run-outs built in crates/brep, several days, live's speed) are in the 22:15 entry above. It
comes back if the fillet speed table shows OCCT's final cut on exact contact edges dominating many fillets.
cf-setback-fillet's SETBACK-FILL-PATCH is blend's from now (lead 11-07 22:30; skasm told in skasm.md).

**11-07 22:40 UTC: fillet speed table (lead's ask).** Every fillet / chamfer of the model suite with a live time, one
run on vibe5 (main 11-07 ~22:00; `NOSHAPE_PARITY_ONLY=<all>` prints per-feature times; script
/root/blend-scratch/bin/blend-table.py, full table /root/blend-scratch/blend-table.txt): 91 features, ours 4584 ms
against live's 2031 in all; 22 slower than live, 69 level or faster. The worst, with what their time is (kernel
profile):

| document › feature | ours | live | where the time goes |
|---|---|---|---|
| cf-snap-hook › Fillet 6 | 187 | 8 | OCCT's fillet: the as-is try fails (14 ms), the scaled retry builds (175) |
| c2-rings-loft › Fillet 1 | 340-488 | 23 | ring_blend's cut: OCCT's edge-face work on the rounds' contact edges (RING-BLEND-SPEED) |
| learning-center-michael › Fillet 2 | 629-694 | 46 | OCCT's fillet falls back to `by_tools` (685; 363 of it face-face intersections) |
| motor-mount-jack › Fillet 2 | 39 | 3 | an OCCT cut (17) and properties |
| c2-rings-loft › Chamfer 1 | 340 | 30 | ring_blend's cut, as Fillet 1 |
| cf-snap-hook › Fillet 5 | 64 | 6 | coaxial_round: its checks (~7 ms an edge) and the glued fuse (38) |
| c2-impeller-loft › Fillet 1 | 374 | 38 | not the fillet (87): resolving its live-id query by geometry before it runs (eval.rs, ~114 `k::distance` against the body, 234 ms): the translator's |
| c2-impeller-loft › Fillet 4 | 541 | 102 | OCCT's fillet falls back to `past_end_faces` (defeaturing, 500) |
| c2-impeller-loft › Fillet Ansaugflansch | 940 | 321 | (not profiled yet) |

So the final cut on exact contact edges (design (b)) is only rings-loft's two features (~700 ms). The bigger share is
OCCT's own fillet falling into its slow fallbacks (the scaled retry, by_tools, past_end_faces: ~1.4 s over three
features): those are next, the slowest first (michael's Fillet 2).

**11-07 23:10 UTC: RABC parked; the speed table made fair; the fallback speeds.**
- SETBACK-FILL-PATCH's RABC parked (lead): one patch, a numerical fit, no quick fix. Reproduced with skasm's
  /root/skasm-scratch/corner/proto.py (RABC 35.66 mm², live 34.464; JcC / JuC within 0.08). Live's RABC is G1 along all
  six edges (0.09° at most), as the other two, so it isn't a position-only edge. Untried idea: Parasolid fixing the
  cross-boundary tangents' magnitudes (from the supports), not only their directions, which a free least-bending fit
  can't show on the two that match.
- Only blends before each document's first difference compare with live (after it the input already differs): 67
  features, ours 2221 ms against live's 773. snap-hook's Fillet 6 (187 ms) fails in ours on a wrong input (Plane 1
  fails upstream, Sweep 1 builds nothing), and the three c2-impeller-loft fillets run after its Loft Gehäuse
  differs: neither is a fair speed case. The fair top: c2-rings-loft Fillet 1 + Chamfer 1 828 ms (live 53),
  learning-center-michael Fillet 2 629 ms (live 46): both a cut of the part by blend-region tools lying on its faces
  (66 % of the comparable blend time). michael's cut: `fillet_by_tools`, declined by noshape's Boolean ("a non-manifold
  result"), OCCT ~600 ms: what the Boolean would need is in bool.md ("From blend 23:10").
- Kernel timers added for the record (not landed): blend.checked.build / .valid / .shrunk, blend.scaled.finish.

## DOCK-FILLET (P1, lead 11-08): AppleWatchDock's "Fis Gecidi R" folds Govde
User data /root/lead-scratch/user-docs/AppleWatchDock.noshape (never in the repo; copy in /root/blend-scratch/watchdock).
Tab "A - Yumusak Kutu", feature index 69 "Fis Gecidi R" (fillet on the USB-C plug passage, after 68 "USB-C Fis Gecidi"
extrude). Probe (not to land): crates/ops/tests/zz_dock_probe.rs (`DOCK=<copy> FROM= TO= NEAR=1 DUMP=<dir>`), and
`NOSHAPE_SI_DUMP=1` in kernel common.cpp `self_intersects` prints the faulty pairs. 01:00: six faulty pairs, mirrored
at x = ±5.5, y ≈ 13.3, z ≈ 9.8 (one shape of each a vertex-sized 0.2 mm box).
- 01:35 UTC: cause and fix. The pairs are vertices with 0.53 / 0.52 / 0.1 mm tolerance overlapping: OCCT's as-is
  fillet puts the corners where the wall underside's round (r 13.95 arc at z 9.9) meets the 0.1 mm step edge's round
  0.5 mm off; tighten_new_tolerances can't tighten them (keeps OCCT's), BRepCheck passes, BOP's check fails. Each
  round alone is fine; together OCCT's corner is wrong. Fix (kernel features.cpp `scaled_blend`): when the as-is
  corners are far off (tighten returns inf) and a loose vertex's ball reaches another vertex
  (`loose_vertices_overlap`), the working-scale build instead (valid, no SI). Dock: Part 1 valid, not SI, volume
  25097.046 mm³ (broken one 25097.085); Fis Gecidi R 102 -> 182 ms (two builds). T1 model
  features_c::rounds_over_a_tenth_mm_step_leave_a_valid_body (stand-in built with the kernel, no user data).
  Live asked (REQUESTS blend 01:21, same stand-in). Left: the subset "outer arc + 0.1 step + outer side" (inner arc
  left out) still SI after the scaled build (a 0.1 mm vertex 0.1 from another); probe /root/blend-scratch/dock/.

## SNAP-FILLET6 (lead 11-08, before BLEND-DIRECT): cf-snap-hook's Fillet 6
R10 on the case's lip, a closed G1 loop of eight concave edges (arcs between coaxial drafted cones, one line between
planes). OCCT's round: a 0.1 mm sliver face where each two arcs meet (90/178/92, live 84/160/80). Fix: when OCCT's
fillet has a sliver (a new face narrower than r / 20 across its size), `fillet_by_tools` without a mitre's ball (a
tangent chain of lines between planes and arcs between coaxial planes / cylinders / cones, all convex or all concave):
exact rounds (tori, the line's cylinder), the tools fused (concave) or cut (convex) in one Glue Shift Boolean. The
document 14 -> 18/18 (regen 4770 -> 3922 ms: Snap Hooks on the right input); Fillet 6 97 -> ~300 ms (live 8: OCCT's
glued Boolean of the tori ~190 ms, the slivered OCCT attempt ~100 first). T1 features_c::a_tangent_loop_of_concave_
rounds_has_no_slivers_as_live (data/snap-hook-lip.brep, Fillet 6's input).
11-08 06:00: such a chain (tangent arcs changing support) goes to the exact rounds first, OCCT's slivered attempt
skipped: Fillet 6 300 -> ~200 ms (the rest is OCCT's glued Boolean of the tori; live 8).
11-08 08:30: the chain's rounds now go through the direct build (blend_direct.cpp, extended: cones / tori by sampled
coincidence, tools taken or added by a point beside their strips, a piece kept by a point just off it, strips inside
the material output, section lines along a face's edge skipped, sections only with the supports' neighbours), the glued
Boolean the fallback: Fillet 6 ~200 -> ~110 ms. rings-loft's ring_blend doesn't use it yet (Shell 1, see BLEND-DIRECT).
Landed 09:10 as 0cb8ba5d. rings-loft through it (branch ring-direct-wip): Fillet 1 ~110 ms, Chamfer 1 ~275 ms, but
Shell 1 fails on the in-memory result and works after a BRep write/read (bool.md "From blend 05:06", update 09:54).
DOCK-CORNER through it (branch blend-direct-wip): the corner tools (trimmed arc, common, vertical) overlap and the 2D
splits don't close yet; next step there: fuse each corner's three small tools into one before the direct build.
11-08 11:10, branch dock-corner2 (WIP, not landed): each corner's tools fused into one (OCCT, ~10 ms a corner), the
direct build then makes the stand-in exactly as live (23/61/40, 8480.50537 mm³; T1 asserts it on the branch). Fixes on
the way: part vertices seeded so cuts ending at a face's corner share its vertex; a section stopping a hair short of
either face's edge (IntSS near a tangency) run on to it; overlaps counted only with real length (two pieces of one
curve touch); a free end run on only for arcs and only where a probe past it is air. In the user's document Fis
Gecidi R now builds directly (Part 1 valid, not self-intersecting, 227 faces) and so do USB Pencere R and Kapak Alt R,
but each is slower than main's OCCT route: 275 / 225 / 517 ms against 60 / 24 / 328 (classifications against the
tools ~0.6 ms each, 50-140 a fillet; sections ~0.5 ms each, 35-100 a fillet). Not landable as a speed regression; the
next levers: classify by 2D position on the tools' strip faces instead of 3D solid classifiers, and skip sections
whose faces only touch along an edge (end caps against the supports' neighbours).
: blends built with no cut (lead 11-07 23:20: design (b))
Why: of the blend time the suite compares with live (67 features before each document's first difference: ours 2221
ms, live 773), two thirds is OCCT cutting the part by blend-region tools whose faces lie on its faces (rings-loft's
Fillet 1 + Chamfer 1 828 ms, live 53; michael's Fillet 2 629 ms, live 46). The rounds themselves are cheap (OCCT's
lone-rim fillet ~9 ms); the cut has to rediscover what we know: which face each touch curve lies on.

Design (stages, each through bool's gate and the before / after suite):
1. **Rims: a plane x a cylinder, round or chamfer** (ring_blend's cases). Per rim, exactly: the spine (where the
   offset plane meets the offset cylinder: an ellipse), the touch curves (the spine moved onto the plane and radially
   onto the cylinder: both ellipses, sharing the spine's angle as parameter), the blend (a chamfer: the ruled surface
   between the touch ellipses, exact as a rational NURBS in that shared parameter; a round: the ball's pipe round the
   spine, a NURBS fitted to it within 1e-7 x size). Trims, all between surfaces we know: a touch curve leaving its
   face (where it crosses that face's other edges: a curve on a plane or cylinder against edges, 2D), two rims' blends
   meeting (one curve; on the mirror plane when the rims mirror each other, else marched in the two rims' angles), a
   blend running out onto another face (marched on the blend against that plane / cylinder). Then the faces: the
   plane and cylinder re-bounded by the touch curves (2D splits of their parameter domains), the blends bounded by
   their curves, the run-out faces split by theirs; sewn, every new edge with exact pcurves. Built in crates/brep
   (geometry + topology in its arenas) and handed to OCCT shapes by the blend bridge (rust_blend.cpp), names and
   history as ring_blend's.
2. **michael's Fillet 2** (fillet_by_tools' cases: lines between planes, arcs between a plane and the cylinder they
   bound, balls at mitres): the same scheme with cylinders, tori and sphere octants as the blends (all exact), the
   mitres where two rounds' axes cross.
Each stage falls back to today's route where its checks fail (a budget only as a hang guard). T1s: rings-loft's ring
(data/rings-loft-ring.brep) against live's numbers, and michael's part.

Stage 1, how (11-08 01:50): the cost is OCCT rediscovering the touch curves on the plane and the bore (the tool's faces
lie on the part's): no Boolean finds them faster, so they're imprinted, not found. Per rim, OCCT's lone-rim blend on
its wedge as now (~9 ms) gives the blend face B with exact edges on the plane and the cylinder. Then, in the kernel
(C++, one call): (1) the intersections that are real: B against each other face of the part it crosses (the outside
the rounds run out onto) and B against the other rims' blends, each a two-face section with pcurves; (2) every
face that gets a curve split in 2D by it (the touch curves on the plane / bore, the run-out curves on the outside, the
section curves on B: BRepFeat_SplitShape, no surface work); (3) the pieces kept by one point each: a part face's piece
outside the removed region (wedge minus its blend), a B piece inside the part; (4) the kept faces sewn into the
solid, history Modified for split faces, Generated from the rim edge for B. Checks: one valid solid whose volume is the part's less
the tools'; else ring_blend's cut as today. Target rings-loft Fillet 1 + Chamfer 1 under ~60 ms.

Stage 1 progress (11-08 02:33, local WIP commit, not landed): kernel `op_blend_direct` (cpp/blend_direct.cpp, Rust
`k::blend_direct`, tried first in `ring_blend`, its cut the fallback). Rings-loft's Fillet 1 builds directly: 9 faces,
719.382 mm³ (live 719.453), T1 ring test passes; Fillet 1 345 -> 114 ms (direct 79: sections 38, tool classifications
16, splits 7, sewing 2). Lessons: BRepAlgoAPI_Section is as slow as the cut (44 ms a pair): GeomInt_IntSS on the faces'
UV boxes instead, each curve trimmed in both faces' 2D domains (a run-out touches the part face's boundary tangentially,
the blend's it crosses square); FClass2d wraps periodic points by default (Perform(uv, false)); a seam is split once and
used both ways; cut edges need SameParameter cleared before BRepLib::SameParameter; two rounds tangent to each other
where they meet the bore: their section stops ~7 µm short, so it's extended to the touch curves' crossing
(GeomLib::ExtendCurveToPoint). Next: Chamfer 1 (free edges after sewing, notch region), then speed (sections, the
tools' classifier), then the suite and bool's gate (no crates/brep change so far).

**11-08 04:06 UTC: BLEND-DIRECT parked (lead: cf-snap-hook Fillet 6 first).** Local WIP commit only (branch state in
/root/projects/noshape-wt/blend, "blend: WIP blends built directly"); stage-2 attempt in
/root/blend-scratch/stage2-by-tools.patch.
- Rings-loft (vibe5): Fillet 1 345 -> ~100 ms and Chamfer 1 276 -> ~195 ms, both built directly, T1 ring test passes
  with live's counts (Fillet 1 719.385 mm³, live 719.453; Chamfer 1 705.021, live 705.853).
- Blocker: the suite's rings-loft drops 12 -> 9: Shell 1 fails ("Could not shell part with selections.", 13-17 s) on
  the direct Fillet 1 result (the chamfer's direct result is fine: with NOSHAPE_BLEND_DIRECT=chamfer the document stays
  12/32). Same faces, edges, lengths and vertices (to 0.05 µm) as the cut's result, same names; bool's quadrics Shell
  repairs both but on ours joins only 2 of 4 split edges and ends "kind 5 status 32" (a wire badly oriented in the
  offset of the x < 4 round). Done so far: touch edges keep OCCT's blend tolerance (Shell reads it as tangency slack),
  EncodeRegularity, section edges as loose as the blend's, unify after: no change. Next idea: dump both results and
  shell them with Shell 1's own parameters in a T1 to diff the offset walls.
- Stage 2 (michael's Fillet 2): the ball tool (box less sphere) has no face on the part, so the direct build declines;
  it needs the sphere's touch circles on the rounds put on directly.
- Dock stand-in vs live 2274: ours 25/68/45, 8480.517 mm³; live 23/61/40, 8480.505 (2 tori + 3 cylinders a side, no
  vertex blends): one face a side more than live (follow-up).
- 05:06: swapping the two rounds' section order changes nothing; the two results' vertex degrees match too. Asked bool
  (bool.md "From blend 05:06") what by_quadrics needs. SNAP-FILLET6 landed (4b3e0611f).

**DOCK-CORNER (follow-up of DOCK-FILLET, 05:15 UTC):** the stand-in (features_c `dock_passage`) after R0.8: ours 25/68/45,
live 23/61/40 (live 2274). Per side ours matches live's 2 tori (14.75 / 15.15) and 3 r0.8 cylinders, including the
step's round running up to z 10.7 (0.9 line) and ending on the inner torus (1.171 curve); the extra face is OCCT's
0.68 mm² B-spline corner patch where the outer arc's torus (15.15) meets the outer mouth's vertical cylinder (axis at
(±6.3, 13.78), on the torus' centre circle): live's cylinder ends on the torus along their intersection curve, no patch.
Next: equal-radius rounds whose spines meet (cylinder axis through the torus' centre circle) mitred, not patched.
- 11-08 06:00, local branch dock-corner-wip: fillet_by_tools extended (lines along cylinders with exact spandrels
  against the circle, arcs at concave corners cut back at the other wall's plane, free ends run on past where only air
  is taken, convex and concave edges in one fillet, an OCCT USD pass after) builds the stand-in exactly as live:
  23/61/40, 8480.50539 mm³ (live 8480.50526). Not landed: in the user's document it also takes two earlier fillets
  and OCCT's Booleans on the 217-face part make them slow (USB Pencere R 55 -> 175 ms, Kapak Alt R 18 -> 254, Fis
  Gecidi R 282 -> 492). Glue Shift is wrong there (tools run on through faces), pre-fusing the tools slower. Needs a
  local Boolean (only the faces the tools touch) or the direct build (BLEND-DIRECT) to be fast enough.
