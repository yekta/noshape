# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 05:58 UTC)

### Landed 11-19
- c75f1894 config-assembly BOM (PartStudio::standard_content; the BOM's feature-property fallback skips it), 12/12.
- 7599cd64 qa session-asm-1119 (per-type mate names, insertion Restore, ui_script markers).
- 24792169 (stack of 2): live2 errors-1119 rows (error flyout, fixed ⚠ on the icons, crossed limits, inactive italic,
  "(Suppressed)", Insert empty studios); Select other reach 8 / 11 px (app-core Viewport::pick_at_pointer_within; the
  test checks 6.5 / 10 / 13.5 px: the boundary moves a pixel between GPUs, fd0740e4 failed on 8); live2
  asm-dialogs-1119: round ⊗, no pointer cross, no hover after a place, white mate dots (InferenceMarks 30 px toward the
  eye, shared with the Part Studio), first-pick disc + instance outline, chip hover QUERY, Group preview errors and ✓
  clearing, "90.000 deg", zero distance last.

### In flight
- asm worktree, branch asm-live1119 (b658a0874, not queued): translator keeps a sketch solve that lands on live's probed
  sketch vertices. bjr with it (vibex8 bjr-probe.out): BOM 22 -> 23/23, 5b44a0 Revolve 2 and bfc58c chamfers fixed;
  Fillet 1 / Washer Face stay (their setData custom feature isn't built: Feature Studios asked of live 11-19 05:20).
  New in that run: 5d8bc1 Sketch 29 "Sketch could not be solved.", Sketch 40, Extrude 21 — baseline (same main
  without the commit, ops-base1119) queued on vibex7 -> asm-keep/bjr-base.out: if they're there too, main's
  (likely skasm 1f9f5ca7e), else mine. Land asm-live1119 only after that.

### Runs
- Night run c (vibex8): xdev-subs 244 -> 266/266 occ, BOM 58 -> 74/74; isaac-ball 86/123 -> 96/128 mates, BOM 60 -> 58/62
  (two "Part 2" rows merged into one qty 2: suspect d12e3025's last fallback giving a taken body; confirm with the
  traced run before changing). infento-subs, scissor-lift still running.
- m19 pair on vibe2 (load 25-29): slow; traced isaac-ball queued after it (isaac-m19.out).

### isaac-ball ranking (stack, 11-19)
- The 7 CAP_EDGE / SWEPT_FACE / INTERSECT connectors are on mates live itself fails ("Mate cannot resolve mate
  connectors."): Revolute 10, Fastened 6, Ball 16, Revolute 15, Fastened 16. Not translator gaps.
- Spiral Bevel gears (1e81ca, 90f78b): GearLab custom feature "#desc" fails ("Failed to fillet selections.") -> no
  part -> both instances and their mates (Revolute 1, 2, Fastened 28, ...) fail. Owner: FeatureScript host — asked the
  lead which.
- Revolute 19 on Part 1 <98> (fa73e1): no edge of ours centred at live's (0, 0, 20 mm); fa73e1 Chamfer 4 query not
  mapped. Mine, next.

### Open (mine)
- Ctrl+V see-through (REQUESTS 03:29 to live2). Insert sort button / third filter and the Measure panel's place
  (REQUESTS 04:39 to live2). The mate face's double outline band (renderer's hovered edges).
- asm-suspension Cap <2> / Cup <1>: live can't resolve them either; rebuild leaves them out (cosmetic).

### Others' items
- psui: asm-dialogs (6) / (8) coplanar depth tie (theirs, 11-19). suspension Body / Joint / LCA: core.
- robot-relations BOM 36/42 (dx); pattern-std BOM 13/17 (scorer rule); welding Part 5 (core); scissor-lift Top <3>.

### Next, in order
1. bjr baseline result -> land asm-live1119 or fix; tell skasm if 5d8bc1 is main's.
2. isaac: the BOM merge (traced run), Revolute 19 / fa73e1.
3. m19 short report; night run c rest.
4. live / live2 answers as they come.

## Scores (last measured, stack / main 11-18)
- bjr-config 137/137 occ (step 2), 31/31 mates, BOM 22/23 before a8dedfc1 (OPG006 Description; rerun). pinslot 42/42, 16/16.
- Light/medium 20 (stack pair 11-18): pattern-std 92/92 mates, BOM 13/17; tvc 12/12 mates, 26/26 BOM; corexy 22/22;
  robotic-arm 31/31, 37/37, 18/18; robot-relations BOM 36/42; suspension 25/28 occ; welding BOM 27/29.

## How to run
- Builds and crate tests: `NOSHAPE_RB_HOST=vibex8 tools/tiers/rb.sh bash -c '…'` (worktree targets warm there;
  rb.sh syncs to vibex8:agents/<worktree>/src; generated docs come back with rsync from there). Grep cargo output for
  `^error|panicked|test result`, dropping `noshape-kernel@` warnings.
- Parity binary: `cargo test -p noshape-ops --test all --no-run` in rb.sh, then relay
  vibex8:agents/<wt>/target/debug/deps/all-<hash> through this machine to vibe2:agents/asm-keep/bin/<name> (vibex8 ->
  vibe2 scp is refused).
- One assembly, traced, via slot: vibe2 `~/agents/asm-keep/one.sh <bin> <asm dir> <out>` (vibex7's one.sh: untraced).
  Suites only on vibe2 / vibex7 (lead 11-18: vibex8 overloaded), one per host.
- Pairs: `~/agents/asm-keep/pairlist.sh <tag> <dirs>` (ops-mainbase vs ops-stack) -> pair/{mainbase,stack}-<tag>.log.
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh` (LAND_BIG_DELETE=1 only for the reviewed standard JSON);
  never from a worktree with commits still queued.
- Worktrees: asm (branch asm-live1119), asm-feel (landed), asm-cursor (landed), asm-docs (docs-only).
- Hosts (lead DECISIONS a67911bef): pick the lowest 1-min load of vibex7 / vibex8 / vibe2 (or the Mac slot) each time.
- Private documents (hat8 / hat15 / dock on vibex7 ~/lead-private): mode-700 copies in ~/tmp/asm only, deleted after.
- Live data: REQUESTS.md (times from `date -u`); live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7, live
  e8549719-ef9a-42de-960c-e24b29f142a6.
- Agent ids: psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, core
  d30002ca-d013-4435-b7d9-7da9cb564cec, blend de66e481-56e2-4005-a5d3-9fa897ca5fde, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, bool 754a9804-feb1-4153-b32a-acf9b2a6d94d
  (old core e8af89fe / blend 0672fa20 stopped: never message them).

## Assembly model suite (what it does)
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over ~/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES).
  `rebuild_assembly`: Part Studios by the translator, each level (subassemblies first, each its own "Sub <eid>" tab)
  inserted, fixed and placed as live's occurrences, mates between the same connectors; top-level instances held at
  live's places, compared, freed (our mates mustn't move them); mate status, BOM rows (parent, depth, name, qty, part
  number, description, material) as live's.
