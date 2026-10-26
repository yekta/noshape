# Lead handoff (read this first if you are the new lead)

## Team from 2026-10-26 19:40 UTC (docs/PLAN.md 2.3; this replaces the older notes below where they differ)

| Agent | Area | Motile id | Account | Worktree / target | Display | Prompt |
|---|---|---|---|---|---|---|
| core | core modelling | 431d971e-f324-4b2b-9a71-d705b920e652 | Yekta | noshape-wt/core, /root/target-core | :101 | /root/lead-scratch/prompt-core.md |
| psui | Part Studio UI | a2602854-3c17-421f-b5b4-37a3c7d03450 | Motile | noshape-wt/psui, /root/target-psui | :102 | /root/lead-scratch/prompt-psui.md |
| skasm | sketch + assembly | 24cb8a1d-753a-4372-8f11-997b8d101c2a | Unbind | noshape-wt/skasm, /root/target-skasm | :103 | /root/lead-scratch/prompt-skasm.md |
| dx | agent API, drawings, data exchange | 0a28d6c8-180a-4c59-8cfe-6bff3f4dcf71 | Stablecog | noshape-wt/dx, /root/target-dx | :105 | /root/lead-scratch/prompt-dx.md |
| live (no builds) | live Onshape research: answers REQUESTS.md, writes specs ahead (live/SPECS.md) | 911412ff-1f48-46de-8008-a4de9c809039 | Motile | Chrome :107 | - | /root/lead-scratch/prompt-live.md |
| qa (no builds) | tests /root/green/noshape vs live, files docs/TICKETS.md | 23cc0f7b-b7e0-47c4-979d-310631c0d2cc | Unbind | noshape-wt/qa (docs only) | :110 | /root/lead-scratch/prompt-qa.md |

All on Opus 5.5. Worktrees are under /root/projects/noshape-wt/ (the repo is /root/projects/noshape). The lead's
watcher tests main in noshape-wt/watch with /root/target-lead; releases are cut in noshape-wt/release. The rustc
throttle (~/.local/bin/rustc-throttle) allows 4 concurrent big-crate compiles plus the watcher's own slot.
Is it working? `bash /tmp/lead/scoreboard.sh 3` (landings per agent per day, reverts, watcher runs, releases).
Check agents with `python3 /tmp/lead/peek.py 4` and their commits (`git log origin/main --grep '^<name>:'`).

You coordinate agents that build noshape (Onshape clone). The user's rules are in docs/HANDOFF.md
("What the user wants") and docs/handoff/TEAM-RULES.md. The most important ones:
- **Exact Onshape look and behaviour, with no deviations.** Only cloud-only features (sign-in, sharing,
  comments, notifications, subscriptions) and simulation/render are excluded. **Never escalate "is this
  OK?" to the user.** Decide reasonably in favour of parity and answer the agent yourself.
- **Quota.** Spread agents over Motile accounts with room (`list_accounts`), and use Opus 5.5 only, never Fable.
  If an agent's account is near or at its limit: when the window resets within about 10 minutes, wait;
  otherwise move the agent. To move one: `stop_agent` (an agent stuck in "waiting_for_usage_limit"
  ignores stop until woken by `message_agent`, so send a message first and stop it right after).
  Then `start_agent` a replacement with a takeover prompt pointing at its handoff file, saying "check git
  diff for half-done edits". Update the agent table in docs/HANDOFF.md.
- **Watch quotas proactively.** Re-create the 15-minute session cron (CronCreate "*/15 * * * *"), which calls
  list_accounts + agent_status for every agent in the HANDOFF.md table and moves stalled ones.
- Send the user side-by-side screenshots (ours | Onshape) as areas land. Agents save them as
  /tmp/*-shots/final-*.png.

## Build load (user, 2026-10-19: "careful with agents that build the app")
- App builds use three group target dirs (2026-10-20, bigger machine): UI → /root/target-shared, behaviour → /root/target-behave, engine → /root/target-engine. Cargo's lock serialises the builds,
  and artifacts are shared. Never give an agent its own target dir again; 7 parallel full builds choked the machine.
- At most ~5 app-building agents at once. Research/capture agents (no builds) don't count. kern and fs are paused for
  load; resume them (message_agent) when a building agent finishes.
- Integration (its own worktree + target-lead) is an extra full build. Run it only at quiet moments, not as an
  endless loop.

## Machinery (all of it keeps running without you)
- **Integration** (`/root/.config/noshape/lead/integrate.sh "<msg>" <tries>`; copy in /tmp/lead/):
  snapshots the shared tree into the worktree /root/noshape-lead, builds, runs `cargo test --workspace
  --exclude noshape` plus the library clippy `-D warnings`, and commits that exact tree to main and pushes when
  everything is green. Run it detached:
  `nohup setsid /tmp/lead/integrate.sh "msg" 200 > /tmp/lead/integrate.log 2>&1 &`.
  Check with `grep -E "try|COMMITTED" /tmp/lead/integrate.log | tail`. It was running at handoff time.
  After a commit, watch CI with `gh run list -R yekta/noshape`.
- **WIP backup:** /tmp/lead/wip-loop.sh (nohup) force-pushes the working tree to branch `wip` every 30 min.
- **Memory throttle:** ~/.cargo/config.toml sets rustc-wrapper=~/.local/bin/rustc-throttle, which allows at most 3
  concurrent compiles of noshape's big crates (15 GB RAM; earlyoom otherwise kills rustc, Chrome and the user's
  k3d coredns).
- **Disk:** keep more than 15 GB free. Delete finished agents' /root/target-* dirs. Only delete `debug/incremental`
  in active ones; agents clean their own stale test binaries (`! -name '*.so'`!). Don't touch the user's other
  projects or worktrees.
- **Releases:** tag `vX.Y.Z` on a green main commit and push the tag; release.yml builds the bundles and publishes
  them. v0.2.0 = 1d2f67c.
- **Strict CI** (fail on any warning): still staged locally in .github/workflows/ci.yml and excluded from
  integration commits. Enable it once the app crate is warning-free.

## Decisions already made (don't revisit)
- FeatureScript replaces Rhai (agent fs). Release workflow is built locally (psb). Onshape-style file tabs
  (asb). Sketch views in drawings (asb). Variable Studio works with Onshape's insertion model (ui2). Deleting a
  tab can't be undone with Ctrl+Z, as in Onshape. The selection-count cursor is a real image cursor, with a
  patched gpui Wayland crate.
- Export formats: all of Onshape's. kern writes them: Parasolid and ACIS are done; Rhino via openNURBS (also
  built in CI/releases, not optional), JT, and PVZ reverse-engineered from Onshape samples. ui2 does
  GLTF/GLB/3MF/URDF.
- Add comment, Ctrl+U feedback and simulation are out of scope as cloud/simulation features.

## Live Onshape reference
/root/onshape-research/live/behaviours.md (has a table of contents). Agents request captures in
live/REQUESTS.md, and the "live" agent answers them. The account is in env vars (see HANDOFF.md).
