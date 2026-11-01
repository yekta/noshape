"""Keep the GitHub Actions cache under its 10 GB quota (ci.yml prune-caches, on main).

Per ref, only the newest cache of each kind stays: a kind is the key without its trailing hashes (rust-cache's
`v0-rust-<key>-<job>-<os>-<arch>-<env>-<lock>`, `occt-<os>-<arch>-<tag>-<patches>`, `occt-ccache-…`,
`opennurbs-…`), so a new Cargo.lock or patch set replaces the old entry instead of sitting beside it until eviction.
Caches of release tags are deleted (no later run can read them), and other branches' caches after three days.

  python3 prune_caches.py [--dry-run]       (GH_TOKEN with actions: write, GITHUB_REPOSITORY)
"""
import datetime
import json
import os
import re
import subprocess
import sys

repo = os.environ["GITHUB_REPOSITORY"]
dry = "--dry-run" in sys.argv


def api(*args):
    out = subprocess.run(["gh", "api", *args], capture_output=True, text=True, check=True).stdout
    return json.loads(out) if out.strip() else None


caches = []
page = 1
while True:
    r = api(f"repos/{repo}/actions/caches?per_page=100&page={page}")
    caches += r["actions_caches"]
    if len(r["actions_caches"]) < 100:
        break
    page += 1


def kind(key):
    return re.sub(r"(-[0-9a-f]{8,})+$", "", key)


now = datetime.datetime.now(datetime.timezone.utc)
newest = {}
for c in caches:
    k = (c["ref"], kind(c["key"]))
    if k not in newest or c["created_at"] > newest[k]["created_at"]:
        newest[k] = c

drop = []
for c in caches:
    age = now - datetime.datetime.fromisoformat(c["last_accessed_at"].replace("Z", "+00:00"))
    if c["ref"].startswith("refs/tags/"):
        drop.append((c, "release tag"))
    elif newest[(c["ref"], kind(c["key"]))] is not c:
        drop.append((c, "replaced"))
    elif c["ref"] != "refs/heads/main" and age > datetime.timedelta(days=3):
        drop.append((c, "old branch"))

mb = lambda n: f"{n / 2**20:,.0f} MB"
total = sum(c["size_in_bytes"] for c in caches)
freed = sum(c["size_in_bytes"] for c, _ in drop)
for c, why in drop:
    print(f"delete {mb(c['size_in_bytes']):>9}  {why:12}  {c['ref']}  {c['key']}")
    if not dry:
        api("-X", "DELETE", f"repos/{repo}/actions/caches/{c['id']}")
print(f"{len(caches)} caches, {mb(total)}; deleted {len(drop)}, {mb(freed)}; {mb(total - freed)} left")
