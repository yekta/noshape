# Lead scripts (copied 2026-10-26 from /tmp/lead; /tmp is wiped on reboot)

Copy to /tmp/lead/ before use (the scripts reference /tmp/lead paths), then run `tools/lead/loops.sh` to start the loops.

| Script | What it does |
|---|---|
| `loops.sh` | Restarts the background loops if they died (exact-match pgrep). |
| `disk-loop.sh` | Every 30 min: deletes stale test binaries, old artifact variants, stale build-script dirs, incremental caches, perf's ~/.debug cache, old scratch binaries. |
| `watch-main.sh` | Tests the newest main (quick pass on touched crates, then full); green moves tag `last-green` (CI runs on it) and copies the app to /root/green; red writes /tmp/lead/main-status. Builds C++ with GCC 13 like CI. **Per docs/PLAN.md, switch it to T0+T1 only.** |
| `cut-release.sh <ver> <sha>` | Cuts release/v<ver> from a green commit, bumps the workspace version, tags v<ver> (release.yml builds and publishes). |
| `peek.py [n]` | Shows each agent's last n actions from its transcript; flags agents idle > 20 min. |
| `menu-cover.py <ref>…` | Approximates the menu-coverage test per ref without building. |
| `prune-build.py`, `prune-deps.py`, `fix-fingerprints.py` | Disk helpers used by disk-loop. |
