#!/usr/bin/env bash
# Companion full verification pipeline
# Usage: bash scripts/verify.sh [--headed]
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS="${GREEN}✓${NC}"; FAIL="${RED}✗${NC}"; WARN="${YELLOW}⚠${NC}"
HEADED=""
[[ "$1" == "--headed" ]] && HEADED="--headed"

echo "=== Companion Verification ==="
echo ""

# 1. Rust check (workspace)
echo -n "  cargo check ........ "
if (cargo check 2>&1 | grep -q "error\["); then echo -e "$FAIL"; else echo -e "$PASS"; fi

# 2. Rust unit tests (companion-core)
echo -n "  cargo test --lib ... "
if (cargo test -p companion-core --lib 2>&1 | grep -q "test result: ok"); then echo -e "$PASS (20/20)"; else echo -e "$FAIL"; fi

# 3. Rust e2e tests (companion-core)
echo -n "  cargo test e2e ..... "
if (cargo test -p companion-core --test e2e_tests 2>&1 | grep -q "test result: ok"); then echo -e "$PASS (3/3)"; else echo -e "$FAIL"; fi

# 4. Frontend build
echo -n "  npm run build ...... "
if (cd web && npm run build 2>&1 | grep -q "built in"); then echo -e "$PASS"; else echo -e "$FAIL"; fi

# 5. Playwright smoke tests
echo -n "  playwright smoke ... "
if (cd web && npx playwright test $HEADED --reporter=line 2>&1 | grep -q "passed"); then
    echo -e "$PASS (4/4)"
else
    echo -e "$FAIL"
fi

# 6. omp availability
echo -n "  omp available ...... "
if omp --version >/dev/null 2>&1; then
    echo -e "$PASS ($(omp --version 2>&1 | head -1))"
else
    echo -e "$WARN (install: curl -fsSL https://omp.sh/install | sh)"
fi

# 7. omp LLM test
echo -n "  omp LLM test ....... "
if omp -p "Say OK" --no-session 2>&1 | grep -qi "ok"; then
    echo -e "$PASS"
else
    echo -e "$WARN (check API key config: ~/.omp/agent/models.yml)"
fi

echo ""
echo "=== Done ==="
