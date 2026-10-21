#!/bin/bash

set -u

WATCHDOG_LABEL="com.robot.coding-tools-mcp-tunnel-watchdog"
TUNNEL_LABEL="${CTMCP_WATCHDOG_TUNNEL_LABEL:-com.robot.cloudflared.coding-tools-mcp}"
SUPPORT_ROOT="${CTMCP_WATCHDOG_SUPPORT_ROOT:-$HOME/Library/Application Support/coding-tools-mcp-desktop}"
STATE_DIR="${CTMCP_WATCHDOG_STATE_DIR:-$SUPPORT_ROOT/watchdog/tunnel}"
LOG_FILE="${CTMCP_WATCHDOG_LOG_FILE:-$SUPPORT_ROOT/logs/tunnel-watchdog.log}"
CURL_BIN="${CTMCP_WATCHDOG_CURL_BIN:-/usr/bin/curl}"
PYTHON_BIN="${CTMCP_WATCHDOG_PYTHON_BIN:-/usr/bin/python3}"
LAUNCHCTL_BIN="${CTMCP_WATCHDOG_LAUNCHCTL_BIN:-/bin/launchctl}"
PROBE_BIN="${CTMCP_WATCHDOG_PROBE_BIN:-}"
FAILURE_THRESHOLD="${CTMCP_WATCHDOG_FAILURE_THRESHOLD:-2}"
COOLDOWN_SECONDS="${CTMCP_WATCHDOG_COOLDOWN_SECONDS:-300}"
MAX_RESTARTS_PER_HOUR="${CTMCP_WATCHDOG_MAX_RESTARTS_PER_HOUR:-3}"
POST_RESTART_ATTEMPTS="${CTMCP_WATCHDOG_POST_RESTART_ATTEMPTS:-4}"
POST_RESTART_WAIT_SECONDS="${CTMCP_WATCHDOG_POST_RESTART_WAIT_SECONDS:-3}"
TARGETS="${CTMCP_WATCHDOG_TARGETS:-quant|http://127.0.0.1:28766/mcp|https://mcp.weorall.com/mcp
news|http://127.0.0.1:28769/mcp|https://news-mcp.weorall.com/mcp}"

FAILURE_FILE="$STATE_DIR/failure-count"
LAST_RESTART_FILE="$STATE_DIR/last-restart"
RESTART_HISTORY_FILE="$STATE_DIR/restart-history"
LOCK_DIR="$STATE_DIR/run.lock"

now_epoch() {
  if [[ "${CTMCP_WATCHDOG_NOW:-}" =~ ^[0-9]+$ ]]; then
    printf '%s\n' "$CTMCP_WATCHDOG_NOW"
  else
    /bin/date +%s
  fi
}

trim_log_if_needed() {
  [ -f "$LOG_FILE" ] || return 0
  local bytes tmp
  bytes=$(/usr/bin/wc -c < "$LOG_FILE" | /usr/bin/tr -d ' ')
  [ "${bytes:-0}" -le 1048576 ] && return 0
  tmp="$LOG_FILE.tmp.$$"
  /usr/bin/tail -n 2000 "$LOG_FILE" > "$tmp" && /bin/mv -f "$tmp" "$LOG_FILE"
}

log_event() {
  /bin/mkdir -p "$(/usr/bin/dirname "$LOG_FILE")"
  trim_log_if_needed
  printf '%s label=%s %s\n' "$(/bin/date -u '+%Y-%m-%dT%H:%M:%SZ')" "$WATCHDOG_LABEL" "$*" >> "$LOG_FILE"
}

read_nonnegative_integer() {
  local file="$1" value="0"
  if [ -f "$file" ]; then
    value=$(/bin/cat "$file" 2>/dev/null || printf '0')
  fi
  if [[ "$value" =~ ^[0-9]+$ ]]; then
    printf '%s\n' "$value"
  else
    printf '0\n'
  fi
}

write_atomic() {
  local file="$1" value="$2" tmp
  tmp="$file.tmp.$$"
  printf '%s\n' "$value" > "$tmp" && /bin/mv -f "$tmp" "$file"
}

acquire_lock() {
  local owner=""
  if /bin/mkdir "$LOCK_DIR" 2>/dev/null; then
    printf '%s\n' "$$" > "$LOCK_DIR/owner"
    return 0
  fi
  if [ -f "$LOCK_DIR/owner" ]; then
    owner=$(/bin/cat "$LOCK_DIR/owner" 2>/dev/null || true)
  fi
  if [[ "$owner" =~ ^[0-9]+$ ]] && /bin/kill -0 "$owner" 2>/dev/null; then
    return 1
  fi
  /bin/rm -f "$LOCK_DIR/owner" 2>/dev/null || true
  /bin/rmdir "$LOCK_DIR" 2>/dev/null || return 1
  /bin/mkdir "$LOCK_DIR" 2>/dev/null || return 1
  printf '%s\n' "$$" > "$LOCK_DIR/owner"
  log_event "event=stale_lock_recovered"
}

release_lock() {
  /bin/rm -f "$LOCK_DIR/owner" 2>/dev/null || true
  /bin/rmdir "$LOCK_DIR" 2>/dev/null || true
}

probe_json() {
  local kind="$1" url="$2" body
  if [ -n "$PROBE_BIN" ]; then
    "$PROBE_BIN" "$kind" "$url"
    return $?
  fi
  body=$(
    "$CURL_BIN" -fsS --connect-timeout 3 --max-time 6 "$url" 2>/dev/null
  ) || return 1
  case "$kind" in
    discovery)
      printf '%s' "$body" | "$PYTHON_BIN" -c \
        'import json,sys; data=json.load(sys.stdin); raise SystemExit(0 if data.get("name")=="coding-tools-mcp" else 1)' \
        >/dev/null 2>&1
      ;;
    oauth)
      printf '%s' "$body" | "$PYTHON_BIN" -c \
        'import json,sys; data=json.load(sys.stdin); value=data.get("authorization_servers"); raise SystemExit(0 if isinstance(value,list) and value else 1)' \
        >/dev/null 2>&1
      ;;
    *) return 2 ;;
  esac
}

oauth_url_for() {
  local public_url="$1"
  printf '%s/.well-known/oauth-protected-resource\n' "${public_url%/mcp}"
}

collect_health() {
  local mode="$1" target local_url public_url oauth_url
  local any_local=0 public_failures=0 checked=0
  while IFS='|' read -r target local_url public_url; do
    [ -n "$target" ] || continue
    if ! probe_json discovery "$local_url"; then
      [ "$mode" = "log" ] && log_event "event=local_unhealthy target=$target"
      continue
    fi
    any_local=1
    checked=$((checked + 1))
    oauth_url=$(oauth_url_for "$public_url")
    if probe_json discovery "$public_url" && probe_json oauth "$oauth_url"; then
      [ "$mode" = "log" ] && log_event "event=target_healthy target=$target"
    else
      public_failures=$((public_failures + 1))
      [ "$mode" = "log" ] && log_event "event=public_unhealthy target=$target"
    fi
  done <<EOF
$TARGETS
EOF
  printf '%s %s %s\n' "$any_local" "$checked" "$public_failures"
}

prune_restart_history() {
  local now="$1" cutoff tmp
  cutoff=$((now - 3600))
  tmp="$RESTART_HISTORY_FILE.tmp.$$"
  if [ -f "$RESTART_HISTORY_FILE" ]; then
    /usr/bin/awk -v cutoff="$cutoff" '$1 ~ /^[0-9]+$/ && $1 >= cutoff { print $1 }' \
      "$RESTART_HISTORY_FILE" > "$tmp"
  else
    : > "$tmp"
  fi
  /bin/mv -f "$tmp" "$RESTART_HISTORY_FILE"
}

restart_allowed() {
  local now="$1" last_restart restart_count
  last_restart=$(read_nonnegative_integer "$LAST_RESTART_FILE")
  if [ "$last_restart" -gt 0 ] && [ $((now - last_restart)) -lt "$COOLDOWN_SECONDS" ]; then
    log_event "event=restart_suppressed reason=cooldown remaining_seconds=$((COOLDOWN_SECONDS - (now - last_restart)))"
    return 1
  fi
  prune_restart_history "$now"
  restart_count=$(/usr/bin/wc -l < "$RESTART_HISTORY_FILE" | /usr/bin/tr -d ' ')
  if [ "${restart_count:-0}" -ge "$MAX_RESTARTS_PER_HOUR" ]; then
    log_event "event=restart_suppressed reason=hourly_limit count=$restart_count"
    return 1
  fi
  return 0
}

restart_tunnel() {
  local now="$1" domain
  domain="gui/$(/usr/bin/id -u)/$TUNNEL_LABEL"
  log_event "event=restart_requested tunnel_label=$TUNNEL_LABEL"
  if "$LAUNCHCTL_BIN" kickstart -k "$domain" >/dev/null 2>&1; then
    printf '%s\n' "$now" >> "$RESTART_HISTORY_FILE"
    write_atomic "$LAST_RESTART_FILE" "$now"
    write_atomic "$FAILURE_FILE" 0
    log_event "event=restart_started tunnel_label=$TUNNEL_LABEL"
    return 0
  fi
  log_event "event=restart_failed tunnel_label=$TUNNEL_LABEL"
  return 1
}

verify_after_restart() {
  local attempt health any_local checked public_failures
  attempt=1
  while [ "$attempt" -le "$POST_RESTART_ATTEMPTS" ]; do
    [ "$POST_RESTART_WAIT_SECONDS" -gt 0 ] && /bin/sleep "$POST_RESTART_WAIT_SECONDS"
    health=$(collect_health quiet)
    read -r any_local checked public_failures <<< "$health"
    if [ "$any_local" -eq 1 ] && [ "$checked" -gt 0 ] && [ "$public_failures" -eq 0 ]; then
      log_event "event=recovered attempt=$attempt"
      return 0
    fi
    attempt=$((attempt + 1))
  done
  log_event "event=still_unhealthy_after_restart attempts=$POST_RESTART_ATTEMPTS"
  return 1
}

run_check() {
  local health any_local checked public_failures failures now
  /bin/mkdir -p "$STATE_DIR"
  if ! acquire_lock; then
    return 0
  fi
  trap release_lock EXIT INT TERM

  health=$(collect_health log)
  read -r any_local checked public_failures <<< "$health"
  if [ "$any_local" -eq 0 ] || [ "$checked" -eq 0 ]; then
    write_atomic "$FAILURE_FILE" 0
    log_event "event=check_skipped reason=no_healthy_local_target"
    return 0
  fi
  if [ "$public_failures" -eq 0 ]; then
    write_atomic "$FAILURE_FILE" 0
    return 0
  fi

  failures=$(read_nonnegative_integer "$FAILURE_FILE")
  failures=$((failures + 1))
  write_atomic "$FAILURE_FILE" "$failures"
  log_event "event=public_failure_count count=$failures affected_targets=$public_failures"
  [ "$failures" -lt "$FAILURE_THRESHOLD" ] && return 0

  now=$(now_epoch)
  if restart_allowed "$now" && restart_tunnel "$now"; then
    verify_after_restart || true
  fi
}

show_status() {
  local failures last_restart restart_count
  /bin/mkdir -p "$STATE_DIR"
  failures=$(read_nonnegative_integer "$FAILURE_FILE")
  last_restart=$(read_nonnegative_integer "$LAST_RESTART_FILE")
  restart_count=0
  [ -f "$RESTART_HISTORY_FILE" ] && restart_count=$(/usr/bin/wc -l < "$RESTART_HISTORY_FILE" | /usr/bin/tr -d ' ')
  printf 'failure_count=%s\nlast_restart=%s\nrestart_history_entries=%s\nlog=%s\n' \
    "$failures" "$last_restart" "$restart_count" "$LOG_FILE"
  [ -f "$LOG_FILE" ] && /usr/bin/tail -n 20 "$LOG_FILE"
}

case "${1:-check}" in
  check) run_check ;;
  status) show_status ;;
  *) printf 'usage: %s [check|status]\n' "$0" >&2; exit 64 ;;
esac
