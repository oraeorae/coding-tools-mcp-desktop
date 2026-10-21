#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WATCHDOG="$SCRIPT_DIR/coding-tools-mcp-tunnel-watchdog.sh"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEST_ROOT"' EXIT

MOCK_PROBE="$TEST_ROOT/mock-probe.sh"
MOCK_LAUNCHCTL="$TEST_ROOT/mock-launchctl.sh"

cat > "$MOCK_PROBE" <<'EOF'
#!/bin/bash
kind="$1"
url="$2"
if [[ "$url" == http://127.0.0.1:* ]]; then
  [ "${MOCK_LOCAL_OK:-1}" = "1" ]
else
  [ "${MOCK_PUBLIC_OK:-1}" = "1" ]
fi
EOF

cat > "$MOCK_LAUNCHCTL" <<'EOF'
#!/bin/bash
printf '%s\n' "$*" >> "$MOCK_LAUNCHCTL_LOG"
exit "${MOCK_LAUNCHCTL_EXIT:-0}"
EOF
chmod +x "$MOCK_PROBE" "$MOCK_LAUNCHCTL"

export CTMCP_WATCHDOG_STATE_DIR="$TEST_ROOT/state"
export CTMCP_WATCHDOG_LOG_FILE="$TEST_ROOT/watchdog.log"
export CTMCP_WATCHDOG_PROBE_BIN="$MOCK_PROBE"
export CTMCP_WATCHDOG_LAUNCHCTL_BIN="$MOCK_LAUNCHCTL"
export CTMCP_WATCHDOG_POST_RESTART_ATTEMPTS=1
export CTMCP_WATCHDOG_POST_RESTART_WAIT_SECONDS=0
export CTMCP_WATCHDOG_TARGETS='test|http://127.0.0.1:28766/mcp|https://mcp.example.test/mcp'
export MOCK_LAUNCHCTL_LOG="$TEST_ROOT/launchctl.log"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

assert_eq() {
  [ "$1" = "$2" ] || fail "expected '$2', got '$1'"
}

restart_count() {
  if [ -f "$MOCK_LAUNCHCTL_LOG" ]; then
    /usr/bin/wc -l < "$MOCK_LAUNCHCTL_LOG" | /usr/bin/tr -d ' '
  else
    printf '0\n'
  fi
}

export MOCK_LOCAL_OK=1 MOCK_PUBLIC_OK=1 CTMCP_WATCHDOG_NOW=1000
"$WATCHDOG" check
assert_eq "$(cat "$CTMCP_WATCHDOG_STATE_DIR/failure-count")" 0
assert_eq "$(restart_count)" 0

export MOCK_PUBLIC_OK=0 CTMCP_WATCHDOG_NOW=1100
"$WATCHDOG" check
assert_eq "$(cat "$CTMCP_WATCHDOG_STATE_DIR/failure-count")" 1
assert_eq "$(restart_count)" 0

export CTMCP_WATCHDOG_NOW=1101
"$WATCHDOG" check
assert_eq "$(restart_count)" 1
assert_eq "$(cat "$CTMCP_WATCHDOG_STATE_DIR/failure-count")" 0

export CTMCP_WATCHDOG_NOW=1110
"$WATCHDOG" check
"$WATCHDOG" check
assert_eq "$(restart_count)" 1
grep -q 'reason=cooldown' "$CTMCP_WATCHDOG_LOG_FILE" || fail 'cooldown was not logged'

export MOCK_LOCAL_OK=0 CTMCP_WATCHDOG_NOW=1500
"$WATCHDOG" check
assert_eq "$(cat "$CTMCP_WATCHDOG_STATE_DIR/failure-count")" 0
assert_eq "$(restart_count)" 1

export MOCK_LOCAL_OK=1 MOCK_PUBLIC_OK=1
mkdir "$CTMCP_WATCHDOG_STATE_DIR/run.lock"
printf '%s\n' "$$" > "$CTMCP_WATCHDOG_STATE_DIR/run.lock/owner"
"$WATCHDOG" check
rm "$CTMCP_WATCHDOG_STATE_DIR/run.lock/owner"
rmdir "$CTMCP_WATCHDOG_STATE_DIR/run.lock"
assert_eq "$(restart_count)" 1

mkdir "$CTMCP_WATCHDOG_STATE_DIR/run.lock"
printf '999999\n' > "$CTMCP_WATCHDOG_STATE_DIR/run.lock/owner"
"$WATCHDOG" check
[ ! -d "$CTMCP_WATCHDOG_STATE_DIR/run.lock" ] || fail 'stale lock was not recovered and released'
grep -q 'event=stale_lock_recovered' "$CTMCP_WATCHDOG_LOG_FILE" || fail 'stale lock recovery was not logged'

export MOCK_PUBLIC_OK=0 CTMCP_WATCHDOG_COOLDOWN_SECONDS=0
for now in 2000 2001 2100 2101; do
  export CTMCP_WATCHDOG_NOW="$now"
  "$WATCHDOG" check
done
assert_eq "$(restart_count)" 3

for now in 2200 2201; do
  export CTMCP_WATCHDOG_NOW="$now"
  "$WATCHDOG" check
done
assert_eq "$(restart_count)" 3
grep -q 'reason=hourly_limit' "$CTMCP_WATCHDOG_LOG_FILE" || fail 'hourly limit was not logged'

python3 - <<'PY' "$CTMCP_WATCHDOG_LOG_FILE"
import sys
path = sys.argv[1]
with open(path, 'a', encoding='utf-8') as handle:
    for index in range(4000):
        handle.write(f"padding-{index:04d}-" + "x" * 300 + "\n")
PY
export MOCK_PUBLIC_OK=1 CTMCP_WATCHDOG_NOW=3000
"$WATCHDOG" check
bytes=$(wc -c < "$CTMCP_WATCHDOG_LOG_FILE" | tr -d ' ')
[ "$bytes" -lt 1048576 ] || fail "bounded log is too large: $bytes"

printf 'PASS: tunnel watchdog tests\n'
