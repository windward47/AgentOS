#!/usr/bin/env bash
# Quick frontend smoke test — starts Vite, takes screenshots, checks for errors.
# Usage: bash scripts/verify-ui.sh [page]
#   page: settings (default), chat, avatar, live2d
set -e

PAGE="${1:-settings}"
PORT=5173
URL="http://localhost:${PORT}"

case "$PAGE" in
  settings) URL="${URL}/#/settings" ;;
  live2d)   URL="${URL}/#/live2d" ;;
  chat)     URL="${URL}/#/" ;;
  avatar)   URL="${URL}/avatar.html" ;;
esac

echo "=== Verify UI: ${PAGE} ==="

# Start Vite in background
echo -n "Starting Vite... "
cd web
npx vite --port $PORT &
VITE_PID=$!
cd ..

# Wait for Vite to be ready
for i in $(seq 1 15); do
  curl -s "http://localhost:${PORT}" > /dev/null 2>&1 && break
  sleep 1
done
echo "done"

# Screenshot
echo -n "Screenshot... "
playwright-cli open "$URL" --headed 2>/dev/null &
sleep 3
playwright-cli screenshot "web/tests/screenshots/verify-${PAGE}.png" 2>/dev/null
echo "saved to web/tests/screenshots/verify-${PAGE}.png"

# Console check
echo -n "Console... "
ERRORS=$(playwright-cli console 2>/dev/null | grep -c "Error\|error\|Uncaught" || true)
if [ "$ERRORS" -gt 0 ]; then
  echo "❌ ${ERRORS} errors found"
  playwright-cli console 2>/dev/null | grep "Error\|error\|Uncaught"
else
  echo "✅ clean"
fi

# Snapshot for structure check
echo "DOM snapshot:"
playwright-cli snapshot 2>/dev/null | head -30

# Cleanup
kill $VITE_PID 2>/dev/null || true
echo ""
echo "=== Done ==="
