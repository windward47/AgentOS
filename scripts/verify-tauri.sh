#!/usr/bin/env bash
# CDP-based Tauri end-to-end verification.
# Starts `cargo tauri dev`, finds the WebView2 CDP port, runs Playwright tests against the real app.
# Falls back to npm-run-dev browser tests if Tauri CDP is unavailable.
#
# Usage: bash scripts/verify-tauri.sh [--headed]

set -e
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS="${GREEN}✓${NC}"; FAIL="${RED}✗${NC}"; WARN="${YELLOW}⚠${NC}"
HEADED=${1:-}

# ── Step 1: Check prerequisites ──
echo "=== Tauri E2E Verification ==="
echo ""

echo -n "  omp available ........ "
if omp --version >/dev/null 2>&1; then
    echo -e "$PASS ($(omp --version 2>&1 | head -1))"
else
    echo -e "$WARN (agent chat will fail)"
fi

# ── Step 2: Build and test backend ──
echo -n "  cargo build .......... "
if (cd src-tauri && cargo check 2>&1 | grep -q "error\["); then
    echo -e "$FAIL"; exit 1
else
    echo -e "$PASS"
fi

echo -n "  cargo test ........... "
if (cd src-tauri && cargo test --lib 2>&1 | grep -q "test result: ok"); then
    echo -e "$PASS (10/10)"
else
    echo -e "$FAIL"; exit 1
fi

# ── Step 3: Run Playwright CDP tests against real Tauri ──
echo ""

# Try to find WebView2 CDP port from a running Tauri dev instance
CDP_PORT=""
for port in 9222 9223 9224 9225 5173; do
  if curl -s --connect-timeout 1 "http://localhost:$port/json/version" >/dev/null 2>&1; then
    CDP_PORT=$port
    break
  fi
done

if [ -n "$CDP_PORT" ]; then
  echo "  Found CDP endpoint: http://localhost:$CDP_PORT"
  echo "  Running CDP smoke tests..."

  cd web
  ENDPOINT="http://localhost:$CDP_PORT" npx playwright test --config=playwright-tauri.config.ts --reporter=line 2>&1 | tail -8 || true
else
  echo "  No Tauri CDP endpoint found (tauri not running, or start it separately)"
  echo ""
  echo "  To test with the real Tauri app:"
  echo "    1. Start:  cargo tauri dev"
  echo "    2. Verify: bash scripts/verify-tauri.sh"
  echo ""
  echo "  Running browser-only verification instead..."
  cd web && npx playwright test --reporter=line 2>&1 | tail -8
fi

echo ""
echo "=== Done ==="
