# noshape workflow (from 2026-10-22 07:00 UTC): own worktree + merge queue, no more freezes

## TRUNK RULES (lead, 10-25 18:50). The merge queue is gone. These override items 3–7, 12 and every ready/queue rule below.
Half the day went to waiting for builds and the queue. Now:
A. **Land straight on main, small and often.** When a small piece of work is done:
   `git fetch origin && git rebase origin/main` (fix conflicts in your files), quick check (B), then `git push origin HEAD:main`.
   If the push is rejected because main moved: fetch, rebase, push again. Don't re-check unless the rebase touched your crates.
   Land every finished piece at once; never sit on more than ~1 h of unlanded work.
B. **Quick check only:** `cargo clippy -p <crates you changed> --all-targets -- -D warnings` (incremental compilation stays OFF: on, its caches
   filled the disk within an hour), plus the one or two tests that cover your change, only if logic changed.
   Never run a crate's whole test suite or the workspace. Docs-only commits: no check.
C. **Commit subjects start with your agent name:** `psb: Hole error text as live`. The watcher uses this to find who broke main.
D. **Main may be red; releases may not.** /tmp/lead/watch-main.sh tests the newest main in the background (build, all
   tests, clippy). Green: it moves the tag `last-green` and copies the app to /root/green/noshape. Red: /tmp/lead/main-status
   lists the failures and the commits since last-green. If your commit broke main, fixing it is your top priority. If main is still red from it 20 minutes after you're told, the lead reverts your commit; you re-land it fixed.
   Check `cat /tmp/lead/main-status` after you push. GitHub CI no longer runs per push: it runs on each new `last-green` tag, on macOS, Windows and Linux, and is never cancelled. Releases are cut only from a `last-green` commit whose CI run passed on all three.
E. **Build the app only to look at it** (a screenshot against live). QA never builds: use /root/green/noshape (last green) or
   build nothing at all. Don't wait for builds you don't need. While anything runs, keep writing code.
F. **Own your files.** Edit only your area (table below). If you need a change in someone else's file, tell the lead the file
   and the exact change; the lead either clears it or has the owner do it.
   | area (files) | owner |
   |---|---|
   | crates/kernel (incl. mesh.cpp except perf's 3 meshing changes) | kern |
   | crates/featurescript, crates/model/src/custom | fs |
   | crates/ops, crates/cli, docs/AGENT_API.md | api |
   | crates/render/src/renderer.rs pipelines, app startup, perf.yml | perf |
   | crates/model/src/regen/features, model features/common, part_studio/behaviors.rs (feature behaviour), manipulators.rs | psb |
   | selection, box select, pre-highlight, measure, mass properties, section view (docs/parity/interaction.md), wherever that code lives | ui (from 10-25 18:00) |
   | crates/app/src/part_studio/mod.rs, feature_dialog, menus, view_tools, shortcuts_page, workspace chrome | ui |
   | icons, crates/scene/src/palette.rs, scene builder overlays, overlay shaders | icons2 |
   | crates/scene/src/sketch_mode.rs visuals, assembly/drawing/start-window visuals | ui2 |
   | crates/sketch, sketch behaviour, assembly_view.rs, model assembly, standard content | skb |
   | QA only, no code outside docs/parity and qa docs (except fixes the lead assigns) | qa5 |

1. Work ONLY in your worktree /root/wt/<you> (branch agent/<you>). Never edit /root/noshape again: it is retired as a shared tree.
   Your branch starts from `transition` = everyone's work as of 06:55 (including your own uncommitted edits). Nothing to port.
2. Build there with your group's target dir as before (OCCT_ROOT=/root/occt CARGO_TARGET_DIR=<group dir>). Copy the binary right after
   building (other worktrees in your group overwrite target/debug/noshape). Kill apps by PID only.
3. Commit small, finished steps on your branch: `git add -A && git commit -m "<what, in plain words>"`.
   A commit must compile, pass `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, and pass the tests of the crates you touched.
4. When a commit is ready to land: `git merge main` (take others' merged work; fix conflicts), re-check, then
   `git branch -f ready/<you> HEAD`.
5. The merge queue (/tmp/lead/mq.sh) merges ready commits onto main every few minutes, builds + runs the workspace tests + clippy,
   and pushes main when green. Your result: `cat /tmp/lead/mq/<you>.status` → MERGED <sha> | FAILED: <errors> | CONFLICT.
   On FAILED/CONFLICT: fix on your branch, mark ready again. A red branch blocks only you.
6. Merge main into your branch often (after each MERGED you see in /tmp/lead/mq/queue.log) so conflicts stay small.
7. QA agents: the newest main build is /root/target-lead/debug/noshape (the queue builds every merged main there). Copy it before running.
8. Never commit to main or push; never rewrite another agent's branch. Shared files are fine to edit: git merges them; resolve conflicts carefully.
9. (09:45) Each worktree has an UNCOMMITTED `.cargo/config.toml` giving our crates (and the vendored gpui crates) a unique
   codegen-units, so worktrees sharing a group target dir no longer reuse each other's stale artifacts (cargo's hashes ignore the
   worktree path; skb hit this). Don't delete or commit it (/.cargo/ is git-excluded). Your first build after this rebuilds our crates once.
   Incremental compilation is off machine-wide (disk); rebuilds are slower but correct.
10. NEVER use `git stash`: the stash is shared by all worktrees, so `stash@{0}` may be another agent's work a second later.
    Park unfinished work with a WIP commit on your own branch instead (`git commit -m "WIP ..."`), and don't mark it ready.
11. Avoid tool-approval prompts (nobody can approve them and the agent hangs): keep scratch files and screenshots under /root/<agent>-scratch, not /tmp.
12. Build budget (10-24): the machine is CPU-bound, with up to 35-min builds while 12 agents compile at once. Check only what you touched: `cargo check`/`clippy -p <crates you changed>` and those crates' tests, plus `--test <name>` for the specific app tests your change affects. Don't run `cargo test --workspace` or full app test suites yourself; the merge queue runs the whole workspace on every ready ref. Never wait on a build with an `until … sleep` loop for more than 20 min without checking that it's actually progressing.
13. Never write or run your own kill/cleanup scripts that match processes by pattern. On 10-24 an empty pattern SIGTERMed every process on the server. Kill only PIDs you started and saved yourself.
14. Commit messages carry NO AI attribution: no "Co-Authored-By: Claude", no "Generated with Claude Code", no 🤖 line. A shared commit-msg hook strips them; never use --no-verify. The merge queue and CI reject any commit that has one.
15. Never run `cargo fmt` or rustfmt on the tree. The repo is not rustfmt-formatted, and one run rewrites 200+ files. Match the surrounding style by hand.
16. Read /root/wt/PRIORITIES.md before choosing your next item. Most-used CAD work first (P0 perf/correctness, P1 core Part Studio, P2 assembly/drawing/import), with P3 (sheet metal, languages, fonts, release mgmt) only when nothing higher is open.
17. Get it right before you test it (the user's rule, 10-24). Tests confirm good code; they don't find it for you.
    - Before editing, read the code you're changing and its callers, and know the invariants.
    - Think through the edge cases.
    - Re-read your own diff as a reviewer before building.
    - Keep each change small and focused.
    - Stay inside your area. To change another agent's files, ask the lead first: most queue failures are two agents editing the same file.
    - Merge main at least every 2 h, and mark ready in small batches, so conflicts stay small.
    - Tests in ci.yml must be deterministic: no wall-clock, fps or exact GPU-pixel thresholds. Those go in perf.yml with medians.
    - A red queue or CI run caused by your branch is your top priority until it's fixed.
18. Keep at most 2–3 copies of the app binary in your scratch dir. The disk loop deletes noshape* binaries over 100 MB older than 12 h from /root/*-scratch and /tmp.
19. Motile can restart your session at any time (it self-updates; 14:04 and 14:05 on 10-25 killed every agent's running build). Run every build or test longer than a minute detached so it survives: `setsid nohup <script> > <log> 2>&1 < /dev/null &`, write a DONE/EXIT line at the end of the log, and poll the log. After any restart, check your logs and `ps` before re-running anything; never start a second copy of a job that is still alive.
20. Target dir groups (10-25 15:50): cargo locks a target dir for a whole build, so agents in one group build one at a time. Groups: target-shared = ui, icons2; target-behave = psb, skb; target-engine = kern, perf, fs; target-qa = api, ui2 (qa5 builds nothing). Use only your group's dir. A cargo with no rustc children for minutes is waiting on that lock, not building.
