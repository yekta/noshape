# psui (Part Studio UI): current state

Updated 2026-11-19 21:03 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: plain `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
  ~/bin/slot only for suites, gates, timing pairs). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
  `dialogs::the_dialogs_matrix_matches_live` (rows filter: DIALOG_ROW=…).
- Scenario / app runs: vibex6, at most 3 app instances (shared with studios). Heavy runs (scenario shards) through
  `~/bin/slot <cmd>` there (one per host, waits for 10 GB free). The app build there OOMs under load: scripts build
  with `-j 4`.
- Land only with `tools/tiers/land.sh` (`--status` for results). Start an rb.sh run, wait until it builds (its sync
  done), then land.sh: land.sh's rebase during an rb.sh sync breaks that build.
- Live: Chrome on :107, CDP 127.0.0.1:9333, helpers /root/live-pw; captures are asked of live2 in
  onshape-research/live/REQUESTS.md (dated `date -u`). Recorded behaviour: onshape-research/live/behaviours.md.
- Private documents: work on a mode-700 copy in ~/tmp, delete it after.
- GPUI tests: platform keys as "secondary-…"; marks (`ui_script::mark`) only record while a scenario runs, so tests
  find controls by `debug_selector`.
- psui owns docs/parity/interaction-feel.md (the feel matrices; all matched as of 11-15): keep it current.
- A new menu label needs its line in crates/ops/src/coverage.rs (`cargo test -p noshape-ops --test all coverage::`).

## Tools (/root/psui-scratch)

- `uip-some.sh <out> <names…>`: release build on the rb host, then the named ui-parity scenarios on one Xvfb (:392).
  Run: `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).
- sbs.py: side-by-side crops (ours offset (242, 90)); mirror-sxs/ holds the Mirror side by sides with README.
- vibe ~/tmp is pruned after 6-12 h: keep results in ~/agents/psui-keep.

## Now (updated 21:03 UTC 11-19; this thread replaced 4973e295 at 13:27, Ubeyd's weekly limit)

1. LANDED 13:59 (e87a2b3e): routing Multiple offset, the edge double-click / region look / constraint refusal
   tests, the section cap (tan only while just picked), the Hole callout size.
2. f117aeaf FAILED 18:20 (menu coverage: the chrome test's literal menu labels); live2 then showed live has NO safe
   triangle (submenu-diag-1119), so the triangle is gone. Re-landing now: qa's P0 feel rows (qa-feel-1119 in
   interaction-feel.md): a left press that closes an open menu gives the graphics nothing (face-rclick-select-fillet-
   faces, the 05:51 regression), one Esc after a field's menu item (LeaveFocus only forward sets tabbed_out), no
   readout under an analysis tool; the untested-row tests; Routing curve Multiple per live2 routing-multi with its
   value box ("10.2 mm" / "45 deg", typed + Enter sets the drag's total, another Edit reference drops it) and the
   arrow drag's blue start line. vibe2 scenarios (~/tmp/psui/fillet-faces): clear-all and clear-missing match.
3. Next: the routing triad's centre ball drag (free, in the view plane); rerun qa's 7 rows on the landed build.
4. asm FYI 11-19 (df6a2c16 queued): the Part Studio Mass panel's "Center of mass ☐ Override" X / Y / Z and "Mass
   moments of inertia ☐ Override" 9 fields (live2 mass-override-1119, behaviours "asm (11-19)"): enabled only for a
   part with a material, Lxy / Lyx not linked; written through doc.edit_studio as menus.rs mass_override
   (PartProperties com_override, inertia_override column by column).
5. Interaction-feel rows that match live with no test (grep "missing" in the Test column; P0 sections first).
   Skipped: the two-block box counts (live's camera: iso + 10 wheel steps about an unknown pointer), the rollback
   bar's hover colour (GPUI group_hover, no test hook), the spinner's 900 ms (`Instant`, not the test clock).
   Asked of live2: the diagonal submenu move's timing (REQUESTS 16:37).
6. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over;
   vibe2 ~/tmp/psui/hv binaries. Branch psui-backup-1119 (all landed; delete).

## Agents (lead 11-19 21:xx; use only these ids)

- lead 818534eb-f6b1-4a00-bb48-ebf0f7cdbd8a; core 08a98761-3bbe-47ce-a91a-32b208f5c6b2; blend 459f6c19-c460-4ebf-96ca-bf92455f2e1e;
  bool abdebc8b-6b39-4794-8390-eb02d6fa37e6 (bool2 merged in); live 349fe81d-db78-4931-8032-6dc3edc5d558 (live2 merged in);
  studios 91339202-41f1-4465-91f4-65ccd26ea1b1; dx 6cfc89bd-1196-43ef-9225-0014546fee1e; qa ac50e7f2-4677-4cab-a57a-fd068a0c07bf;
  asm d8d597e0-675c-497b-a804-544ceab99bbd; skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6. Context cap 200k (user 11-19).
- P0 sketch-vertical-key (lead after compaction: "match live's default view orientation, re-baseline"): checked 21:1x:
  ours already opens in live's start view, Camera::default = Trimetric (-0.433, 0.75, -0.5) = live's recorded start
  camera direction (every live take with a camera); live's Isometric menu item is the true (-1, 1, -1)/√3, as ours
  (view-cube-iso). The (64, -45) vs (61, -42) gap is the zoom of the camera-less 11-06 take (~0.2885 mm/px implied;
  live's empty start is 0.2754 in 17 of 25 takes, 0.2507 in today's). live2 retook it with its camera (cf70e676):
  live shows no Vertical at 0.2507. NEXT: once cf70e676 lands, run sketch-vertical-key on ours (uip-shots.sh); if ours
  infers Vertical on live's camera, the gap is the reach measure (screen distance to the projected plumb line) — with
  skasm. No orientation change, no re-baseline unless that run shows one.

## Open decisions

- None.
