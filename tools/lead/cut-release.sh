#!/bin/bash
# Daily release train (docs/PLAN.md 2.2), in two steps so nothing is tagged before CI passes on all three platforms:
#   cut-release.sh branch <version> <sha> [<fix sha>...]  release/v<version> from a last-green commit, workspace version
#                                                          bumped (Cargo.toml + our crates in Cargo.lock), fixes cherry-picked;
#                                                          pushing the branch runs CI (T1 on Linux, macOS, Windows).
#   cut-release.sh tag <version>                           after that CI run passed: tag v<version> on the branch head;
#                                                          release.yml builds the bundles and publishes the release.
set -eu
W=/root/projects/noshape-wt/release; G="git -c user.name=yekta -c user.email=ngyekta@gmail.com"
cd $W; git fetch -q origin --tags -f
case "$1" in
branch)
  V=$2; S=$3; shift 3
  git checkout -q -B release/v$V $S
  for f in "$@"; do $G cherry-pick -x $f >/dev/null; done
  old=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
  sed -i "0,/^version = \"$old\"/s//version = \"$V\"/" Cargo.toml
  python3 - "$old" "$V" <<'P'
import re,sys
old,new=sys.argv[1],sys.argv[2]; p='Cargo.lock'; s=open(p).read()
s=re.sub(r'(name = "noshape[^"]*"\nversion = )"'+re.escape(old)+'"', r'\1"'+new+'"', s)
open(p,'w').write(s)
P
  git add Cargo.toml Cargo.lock; $G commit -q -m "Release v$V"
  git push -q -f origin release/v$V
  echo "pushed release/v$V at $(git rev-parse --short HEAD) (from $S, version $old -> $V); tag it once CI on the branch is green"
  git checkout -q --detach origin/main;;
tag)
  V=$2; git checkout -q --detach origin/release/v$V
  git tag -f v$V; git push -q origin v$V
  echo "tagged v$V at $(git rev-parse --short HEAD); release.yml publishes"
  git checkout -q --detach origin/main;;
*) echo "usage: cut-release.sh branch <version> <sha> [<fix sha>...] | tag <version>"; exit 2;;
esac
