#!/bin/sh
# Installs noshape from this folder (the release tar.gz): the `noshape` binary (the app, the CLI and the MCP server
# `noshape mcp`), its .desktop entry and icons; then offers to connect noshape to the coding agents found (Claude Code,
# Codex CLI).
#
#   ./install.sh               for you: ~/.local/bin, ~/.local/share
#   ./install.sh --system      for everyone: /usr/local/bin, /usr/local/share (uses sudo when not root)
#   ./install.sh --agents      also connect the coding agents found, without asking (--no-agents: don't ask)
#   ./install.sh --uninstall   remove what --system / the default put there (with --system for the system one)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
system=0 agents=ask uninstall=0
for a in "$@"; do
  case "$a" in
    --system) system=1 ;;
    --agents) agents=yes ;;
    --no-agents) agents=no ;;
    --uninstall) uninstall=1 ;;
    -h|--help) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "install.sh: unknown option $a (see --help)" >&2; exit 2 ;;
  esac
done

if [ $system = 1 ]; then
  bin=/usr/local/bin share=/usr/local/share
  sudo=""; [ "$(id -u)" = 0 ] || sudo=sudo
else
  bin="$HOME/.local/bin" share="${XDG_DATA_HOME:-$HOME/.local/share}"
  sudo=""
fi
apps="$share/applications" icons="$share/icons/hicolor"
# Agents are the user's: under `sudo ./install.sh`, their configuration is the invoking user's, not root's.
as_user() { if [ "$(id -u)" = 0 ] && [ -n "${SUDO_USER:-}" ]; then sudo -u "$SUDO_USER" -H "$@"; else "$@"; fi; }

refresh() {
  command -v update-desktop-database >/dev/null 2>&1 && $sudo update-desktop-database -q "$apps" 2>/dev/null || true
  command -v gtk-update-icon-cache >/dev/null 2>&1 && $sudo gtk-update-icon-cache -q -t "$icons" 2>/dev/null || true
}

if [ $uninstall = 1 ]; then
  [ -x "$bin/noshape" ] && [ "$agents" != no ] && as_user "$bin/noshape" agents disconnect >/dev/null 2>&1 || true
  $sudo rm -f "$bin/noshape" "$apps/noshape.desktop" "$icons/512x512/apps/noshape.png" "$icons/256x256/apps/noshape.png" "$icons/scalable/apps/noshape.svg"
  refresh
  echo "noshape removed from $bin and $share"
  exit 0
fi

$sudo mkdir -p "$bin" "$apps" "$icons/512x512/apps" "$icons/scalable/apps"
$sudo install -m 755 "$here/noshape" "$bin/noshape"
# The menu entry starts the installed binary by its full path (a session's PATH may not have ~/.local/bin).
sed "s|^Exec=noshape|Exec=$bin/noshape|" "$here/noshape.desktop" > "${TMPDIR:-/tmp}/noshape.desktop.$$"
$sudo install -m 644 "${TMPDIR:-/tmp}/noshape.desktop.$$" "$apps/noshape.desktop"
rm -f "${TMPDIR:-/tmp}/noshape.desktop.$$"
$sudo install -m 644 "$here/noshape.png" "$icons/512x512/apps/noshape.png"
$sudo install -m 644 "$here/noshape.svg" "$icons/scalable/apps/noshape.svg"
refresh
# The app links the desktop's libraries (X11/Wayland keyboard, fonts, Vulkan): say which are missing.
if ! version=$("$bin/noshape" --version 2>/dev/null); then
  echo "noshape is installed to $bin/noshape but can't start: these libraries are missing:" >&2
  ldd "$bin/noshape" | sed -n 's/^\s*\(\S*\) => not found/  \1/p' >&2
  echo "Debian/Ubuntu: sudo apt install libxkbcommon0 libxkbcommon-x11-0 libxcb1 libx11-xcb1 libfreetype6 libfontconfig1 libwayland-client0 libvulkan1" >&2
  echo "(or install noshape-<version>-linux-x86_64.deb, which pulls them in). Then run ./install.sh again." >&2
  exit 1
fi
echo "Installed $version to $bin/noshape"
case ":$PATH:" in
  *":$bin:"*) ;;
  *) echo "Note: $bin isn't on your PATH. Add this line to ~/.profile (or your shell's rc file), then open a new terminal:"
     echo "  export PATH=\"$bin:\$PATH\"" ;;
esac

# Coding agents: the MCP server registered with each one found (`noshape agents connect`).
found=$(as_user "$bin/noshape" agents status | sed -n 's/.*"found_names": "\(.*\)".*/\1/p')
if [ -z "$found" ]; then
  [ "$agents" = no ] || echo "No coding agent found (Claude Code, Codex CLI). Later: noshape agents connect"
  exit 0
fi
if [ "$agents" = ask ]; then
  if [ -t 0 ]; then
    printf 'Connect noshape to %s (its MCP server, `noshape mcp`)? [Y/n] ' "$found"
    read -r answer || answer=n
    case "$answer" in [nN]*) agents=no ;; *) agents=yes ;; esac
  else
    agents=no
  fi
fi
if [ "$agents" = yes ]; then
  as_user "$bin/noshape" agents connect --command "$bin/noshape" | sed -n 's/.*"name": "\(.*\)".*/  \1/p; s/.*"result": "\(.*\)".*/    \1/p'
else
  echo "To connect $found later: noshape agents connect"
fi
