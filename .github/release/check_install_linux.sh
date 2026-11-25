#!/bin/sh
# check_install_linux.sh <unpacked tarball folder>: install.sh makes Noshape the app for .noshape files on a stock desktop
# (an account without ~/.config, no xdg-mime nor update-desktop-database: qa LINUX-MIME-ASSOC-HARDEN) and with the
# tools there (a user's mimeapps.list kept); --uninstall takes it back. Each run in a scratch home.
set -eu
dir=$(cd "$1" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { echo "check_install_linux: $*" >&2; exit 1; }
# A PATH without xdg-mime and update-desktop-database.
mkdir "$work/bare"
for f in /usr/local/bin/* /usr/bin/* /bin/*; do
  n=$(basename "$f")
  case "$n" in xdg-mime|update-desktop-database) ;; *) [ -e "$work/bare/$n" ] || ln -s "$f" "$work/bare/$n" ;; esac
done
run() { # home path args...
  h=$1 p=$2; shift 2
  env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$h" PATH="$p" "$dir/install.sh" --no-agents "$@" > "$h.log" 2>&1 || { cat "$h.log" >&2; fail "install.sh $* failed"; }
}
check_default() { # home
  grep -qx '\[Default Applications\]' "$1/.config/mimeapps.list" || fail "no [Default Applications] in $1/.config/mimeapps.list"
  grep -q '^application/x-noshape=noshape.desktop;\{0,1\}$' "$1/.config/mimeapps.list" || fail "no .noshape default in $1/.config/mimeapps.list"
  if command -v gio >/dev/null 2>&1; then
    HOME="$1" gio mime application/x-noshape 2>/dev/null | grep -q 'noshape.desktop' || fail "gio: no default app for application/x-noshape"
  fi
}

# A stock desktop: no ~/.config, neither tool.
h="$work/stock"; mkdir "$h"
run "$h" "$work/bare"
check_default "$h"
grep -qx 'application/x-noshape=noshape.desktop;' "$h/.local/share/applications/mimeinfo.cache" || fail "noshape.desktop not indexed in mimeinfo.cache"
run "$h" "$work/bare" --uninstall
if grep -q 'x-noshape' "$h/.config/mimeapps.list" "$h/.local/share/applications/mimeinfo.cache"; then fail "--uninstall left the association"; fi

# With the tools there (when installed) and a user's own mimeapps.list: theirs kept, ours added.
h="$work/user"; mkdir -p "$h/.config"
printf '[Added Associations]\ntext/plain=org.gnome.TextEditor.desktop;\n\n[Default Applications]\ntext/plain=org.gnome.TextEditor.desktop\n' > "$h/.config/mimeapps.list"
run "$h" "$PATH"
check_default "$h"
[ "$(grep -c '^text/plain=org.gnome.TextEditor.desktop' "$h/.config/mimeapps.list")" = 2 ] || fail "the user's own mimeapps.list entries lost"
[ "$(grep -c '^application/x-noshape=' "$h/.config/mimeapps.list")" = 1 ] || fail "more than one .noshape default"
run "$h" "$PATH" --uninstall
if grep -q 'x-noshape' "$h/.config/mimeapps.list"; then fail "--uninstall left the default"; fi
echo "check_install_linux: ok"
