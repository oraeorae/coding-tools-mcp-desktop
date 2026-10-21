#!/bin/bash

set -euo pipefail

LABEL="com.robot.coding-tools-mcp-tunnel-watchdog"
SOURCE_DIR="$(cd "$(dirname "$0")" && pwd)"
SOURCE_SCRIPT="$SOURCE_DIR/coding-tools-mcp-tunnel-watchdog.sh"
SUPPORT_ROOT="$HOME/Library/Application Support/coding-tools-mcp-desktop"
RUNTIME_DIR="$SUPPORT_ROOT/bin"
RUNTIME_SCRIPT="$RUNTIME_DIR/coding-tools-mcp-tunnel-watchdog.sh"
PLIST_PATH="$HOME/Library/LaunchAgents/$LABEL.plist"
DOMAIN="gui/$(id -u)"

install_watchdog() {
  mkdir -p "$RUNTIME_DIR" "$SUPPORT_ROOT/logs" "$SUPPORT_ROOT/watchdog/tunnel" "$HOME/Library/LaunchAgents"
  /usr/bin/install -m 0755 "$SOURCE_SCRIPT" "$RUNTIME_SCRIPT"

  plist_tmp="$PLIST_PATH.tmp.$$"
  {
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
    printf '%s\n' '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
    printf '%s\n' '<plist version="1.0">'
    printf '%s\n' '<dict>'
    printf '%s\n' '  <key>Label</key>' "  <string>$LABEL</string>"
    printf '%s\n' '  <key>ProgramArguments</key>' '  <array>' "    <string>$RUNTIME_SCRIPT</string>" '    <string>check</string>' '  </array>'
    printf '%s\n' '  <key>RunAtLoad</key>' '  <true/>'
    printf '%s\n' '  <key>StartInterval</key>' '  <integer>60</integer>'
    printf '%s\n' '  <key>ProcessType</key>' '  <string>Background</string>'
    printf '%s\n' '  <key>StandardOutPath</key>' '  <string>/dev/null</string>'
    printf '%s\n' '  <key>StandardErrorPath</key>' '  <string>/dev/null</string>'
    printf '%s\n' '</dict>' '</plist>'
  } > "$plist_tmp"
  /usr/bin/plutil -lint "$plist_tmp" >/dev/null
  /bin/mv -f "$plist_tmp" "$PLIST_PATH"

  /bin/launchctl bootout "$DOMAIN/$LABEL" >/dev/null 2>&1 || true
  /bin/launchctl bootstrap "$DOMAIN" "$PLIST_PATH"
  /bin/launchctl kickstart "$DOMAIN/$LABEL"
  printf 'installed=%s\nscript=%s\nplist=%s\n' "$LABEL" "$RUNTIME_SCRIPT" "$PLIST_PATH"
}

uninstall_watchdog() {
  /bin/launchctl bootout "$DOMAIN/$LABEL" >/dev/null 2>&1 || true
  [ ! -f "$PLIST_PATH" ] || /bin/rm -f "$PLIST_PATH"
  [ ! -f "$RUNTIME_SCRIPT" ] || /bin/rm -f "$RUNTIME_SCRIPT"
  printf 'uninstalled=%s\n' "$LABEL"
}

case "${1:-install}" in
  install) install_watchdog ;;
  uninstall) uninstall_watchdog ;;
  *) printf 'usage: %s [install|uninstall]\n' "$0" >&2; exit 64 ;;
esac
