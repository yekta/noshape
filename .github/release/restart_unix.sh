#!/usr/bin/env bash
# check_restart.py on macOS (the previous .app in /Applications, from its dmg) and Linux (the previous tar.gz's
# install.sh, then the previous .deb; under Xvfb).
#   restart_unix.sh <previous release> <new release>
set -euo pipefail
from=$1 want=$2
here=$(cd "$(dirname "$0")" && pwd)
shots=$RUNNER_TEMP/restart-shots
work=$RUNNER_TEMP/restart && mkdir -p "$work" "$HOME/Documents"
python3 -m venv "$work/venv" && "$work/venv/bin/pip" install -q pillow
py=$work/venv/bin/python
check() { "$py" "$here/check_restart.py" --want "$want" --log "$work/update.log" --shots "$shots" "$@"; }
dl() { gh release download "v$from" --repo "$GITHUB_REPOSITORY" --pattern "$1" --dir "$work" --clobber; }
status=0
if [ "$(uname)" = Darwin ]; then
  dl "noshape-$from-macos-arm64.dmg"
  hdiutil attach -nobrowse -readonly -mountpoint "$work/dmg" "$work/noshape-$from-macos-arm64.dmg"
  rm -rf /Applications/noshape.app && cp -R "$work"/dmg/*.app /Applications/noshape.app
  hdiutil detach "$work/dmg"
  doc="$HOME/Documents/update check.noshape"
  check --exe /Applications/noshape.app/Contents/MacOS/noshape --doc "$doc" --label 'macOS app' -- \
    open -n /Applications/noshape.app --env "NOSHAPE_UPDATE_LOG=$work/update.log" --env NOSHAPE_NO_SETUP_PROMPT=1 --args "$doc" || status=1
  pkill -x noshape || true
  exit $status
fi
sudo apt-get update -q
sudo apt-get install -y -q --no-install-recommends xvfb xdotool mesa-vulkan-drivers libvulkan1 pkexec polkitd > /dev/null
Xvfb :99 -screen 0 1280x800x24 > /dev/null 2>&1 &
export DISPLAY=:99
sleep 2
# The previous .deb first pulls in the desktop libraries the tar.gz's binary needs too.
dl "noshape-$from-linux-x86_64.deb"
sudo apt-get install -y -q "$work/noshape-$from-linux-x86_64.deb" > /dev/null
sudo apt-get remove -y -q noshape > /dev/null
dl "noshape-$from-linux-x86_64.tar.gz"
tar xzf "$work/noshape-$from-linux-x86_64.tar.gz" -C "$work"
"$(ls -d "$work"/noshape-"$from"-linux-x86_64*/ | head -1)install.sh"
doc="$HOME/Documents/update check.noshape"
check --exe "$HOME/.local/bin/noshape" --doc "$doc" --label 'Linux install.sh' -- "$HOME/.local/bin/noshape" "$doc" || status=1
pkill -x noshape || true
"$HOME/.local/bin/noshape" --version > /dev/null && "$(ls -d "$work"/noshape-"$from"-linux-x86_64*/ | head -1)install.sh" --uninstall || true
# The .deb: its update runs `pkexec apt-get install`; polkit lets this user through without a password (the runner
# has no one to type it), as a desktop's prompt would once answered.
echo 'polkit.addRule(function(action, subject) { if (subject.user == "'"$USER"'") return polkit.Result.YES; });' | sudo tee /etc/polkit-1/rules.d/49-noshape-update-check.rules > /dev/null
sudo apt-get install -y -q "$work/noshape-$from-linux-x86_64.deb" > /dev/null
check --exe /usr/bin/noshape --doc "$doc" --label 'Linux .deb' -- /usr/bin/noshape "$doc" || status=1
pkill -x noshape || true
exit $status
