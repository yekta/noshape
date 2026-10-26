#!/bin/bash
# cut-release.sh <version> <sha>: release/v<version> from a last-green commit whose 3-platform CI passed; bump the
# workspace version (Cargo.toml + our crates in Cargo.lock), tag v<version> -> release.yml builds and publishes.
set -eu
V=$1; S=$2; W=/root/projects/noshape-wt/release
cd $W; git fetch -q origin --tags -f; git checkout -q -B release/v$V $S
old=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
sed -i "0,/^version = \"$old\"/s//version = \"$V\"/" Cargo.toml
python3 - "$old" "$V" <<'P'
import re,sys
old,new=sys.argv[1],sys.argv[2]; p='Cargo.lock'; s=open(p).read()
s=re.sub(r'(name = "noshape[^"]*"\nversion = )"'+re.escape(old)+'"', r'\1"'+new+'"', s)
open(p,'w').write(s)
P
git add Cargo.toml Cargo.lock
git -c user.name=yekta -c user.email=ngyekta@gmail.com commit -q -m "Release v$V"
git tag -f v$V
git push -q origin release/v$V
git push -q origin v$V
echo "pushed release/v$V and tag v$V at $(git rev-parse --short HEAD) (from $old)"
git checkout -q --detach origin/main
