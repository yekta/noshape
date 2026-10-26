# Lead scripts (copied 2026-10-26 from /tmp/lead; /tmp is wiped on reboot)

Copy to /tmp/lead/ before use (the scripts reference /tmp/lead paths), then run `tools/lead/loops.sh` to start the loops.

| Script | What it does |
|---|---|
| `loops.sh` | Restarts the watcher and disk loop if they died (exact-match pgrep); fast-forwards /root/projects/noshape's local main to origin/main. |
| `disk-loop.sh` | Every 30 min: deletes stale test binaries, old artifact variants, stale build-script dirs, incremental caches, perf's ~/.debug cache, old scratch binaries. |
| `watch-main.sh` | Tests the newest main with T0 + T1 + clippy (docs/TESTS.md) in /root/projects/noshape-wt/watch; green moves tag `last-green` (CI runs T1 on it on 3 platforms) and copies the app to /root/green; red writes /tmp/lead/main-status and reverts the commit at once when it is the only one since last-green. Nightly T2 on last-green after 01:00 UTC (/tmp/lead/t2-status). Builds C++ with GCC 13 like CI. |
| `cut-release.sh <ver> <sha>` | Cuts release/v<ver> from a green commit, bumps the workspace version, tags v<ver> (release.yml builds and publishes). |
| `peek.py [n]` | Shows each agent's last n actions from its transcript; flags agents idle > 20 min. |
| `menu-cover.py <ref>…` | Approximates the menu-coverage test per ref without building. |
| `prune-build.py`, `prune-deps.py`, `fix-fingerprints.py` | Disk helpers used by disk-loop. |
