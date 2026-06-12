#!/usr/bin/env bash
# Companion UI Debug Helper — launch Vite + playwright-cli for live debugging
# Usage: bash scripts/debug-ui.sh [--headed]
set -e

HEADED="${1:---headed}"

echo "=== Companion UI Debug ==="
echo ""

# 1. Start Vite dev server in background
echo "Starting Vite dev server..."
cd web
npx vite --port 5173 &
VITE_PID=$!
cd ..
sleep 2

# 2. Open the debug target
TARGET="http://localhost:5173"
echo ""
echo "Available targets:"
echo "  1) Main chat UI  — $TARGET"
echo "  2) Avatar window — $TARGET/avatar.html"
echo ""
read -p "Pick target (1 or 2, default 1): " choice
if [[ "$choice" == "2" ]]; then
  URL="$TARGET/avatar.html"
else
  URL="$TARGET"
fi

echo ""
echo "Opening $URL $HEADED ..."
playwright-cli open "$URL" $HEADED

# Cleanup
kill $VITE_PID 2>/dev/null || true
echo "Done."
