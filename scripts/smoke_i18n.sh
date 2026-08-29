#!/usr/bin/env bash
# Platform-wide smoke checks for the 13-locale i18n skeleton.
# Run from repo root: bash scripts/smoke_i18n.sh
# Exit 0 only if every check passes (SKIPPED checks do not fail).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

PASS=0; FAIL=0; SKIP=0; TOTAL=0
declare -a FAIL_MSGS=()

record() { # record <status> <name> [reason...]
    local status="$1"; shift
    local name="$1"; shift
    TOTAL=$((TOTAL + 1))
    case "$status" in
        PASS) PASS=$((PASS + 1)); echo "[PASS] $name" ;;
        FAIL) FAIL=$((FAIL + 1)); echo "[FAIL] $name — $*"; FAIL_MSGS+=("$name: $*") ;;
        SKIP) SKIP=$((SKIP + 1)); echo "[SKIP] $name — $*" ;;
    esac
}

run_flutter() { # run_flutter <dir> <name> <command...> ; runs in subshell, captures output
    local dir="$1"; shift
    local name="$1"; shift
    local log="$LOG_DIR/$(echo "$name" | tr ' /' '__').log"
    if (cd "$dir" && "$@" >"$log" 2>&1); then
        record PASS "$name"
    else
        record FAIL "$name" "see $log"
    fi
}

# 1. Shared arb key consistency (13 locales)
if python3 "$ROOT/scripts/check_l10n.py" >"$LOG_DIR/arb.log" 2>&1; then
    record PASS "shared arb key consistency (13 locales)"
else
    record FAIL "shared arb key consistency (13 locales)" "$(tail -1 "$LOG_DIR/arb.log")"
fi

# 2. HarmonyOS resource key consistency (admin + client)
if python3 "$ROOT/scripts/check_l10n.py" --harmony \
        "$ROOT/apps/admin/harmonyos/entry/src/main/resources" \
        "$ROOT/apps/client/harmonyos/entry/src/main/resources" \
        >"$LOG_DIR/harmony.log" 2>&1; then
    record PASS "harmonyOS resource key consistency (admin + client)"
else
    record FAIL "harmonyOS resource key consistency (admin + client)" "$(tail -1 "$LOG_DIR/harmony.log")"
fi

# 3/4. Flutter tests (admin, client)
run_flutter "$ROOT/apps/admin/flutter" "flutter test (admin)" flutter test
run_flutter "$ROOT/apps/client/flutter" "flutter test (client)" flutter test

# 5/6. Flutter web builds + artifact presence
run_flutter "$ROOT/apps/admin/flutter" "flutter build web (admin)" flutter build web
if [ -f "$ROOT/apps/admin/flutter/build/web/index.html" ]; then
    record PASS "web artifact index.html (admin)"
else
    record FAIL "web artifact index.html (admin)" "build/web/index.html missing"
fi

run_flutter "$ROOT/apps/client/flutter" "flutter build web (client)" flutter build web
if [ -f "$ROOT/apps/client/flutter/build/web/index.html" ]; then
    record PASS "web artifact index.html (client)"
else
    record FAIL "web artifact index.html (client)" "build/web/index.html missing"
fi

# 7. HarmonyOS .hap build artifacts (SKIPPED if cleaned, not a failure)
hap_ok=1; hap_missing=""
for side in admin client; do
    hap="$ROOT/apps/$side/harmonyos/entry/build/default/outputs/default/entry-default-unsigned.hap"
    [ -f "$hap" ] || { hap_ok=0; hap_missing="$hap_missing $side"; }
done
if [ "$hap_ok" = 1 ]; then
    record PASS "harmonyOS .hap artifacts (admin + client)"
else
    record SKIP "harmonyOS .hap artifacts (admin + client)" "missing:$hap_missing — rerun Task 3 build (devEco/hvigor) to regenerate"
fi

# 8. Summary
echo
echo "==== smoke_i18n summary: $PASS/$TOTAL PASS, $FAIL FAIL, $SKIP SKIP ===="
if [ "$FAIL" -gt 0 ]; then
    printf 'Failed checks:\n'
    for m in "${FAIL_MSGS[@]}"; do printf '  - %s\n' "$m"; done
    exit 1
fi
exit 0
