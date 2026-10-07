#!/usr/bin/env bash
# Window smoke test: opens the real Halberd window on a virtual screen,
# takes a screenshot, quits through File > Quit, and checks that the panel
# layout was saved.
#
# Usage (needs Xvfb, xdotool, ImageMagick and a Vulkan driver such as Mesa's
# lavapipe):
#   xvfb-run -s "-screen 0 1600x900x24" tools/smoke/window-smoke.sh <halberd binary> [output folder]
#
# The screenshot lands in the output folder so it can be looked at in CI.
set -euo pipefail

BIN="${1:?usage: window-smoke.sh <halberd binary> [output folder]}"
OUT="${2:-smoke-output}"
mkdir -p "$OUT"
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"

# A throwaway home folder, so the test starts like a first launch and never
# touches real settings.
TEST_HOME="$(mktemp -d)"
export HOME="$TEST_HOME"
export XDG_CONFIG_HOME="$TEST_HOME/.config"
export XDG_DATA_HOME="$TEST_HOME/.local/share"

"$BIN" >"$OUT/halberd.log" 2>&1 </dev/null &
PID=$!

fail() {
    echo "SMOKE TEST FAILED: $1"
    echo "--- Halberd output ---"
    cat "$OUT/halberd.log" || true
    kill "$PID" 2>/dev/null || true
    exit 1
}

# Wait up to 60 seconds for the window to appear.
for _ in $(seq 1 60); do
    if xdotool search --name "Halberd Map Editor" >/dev/null 2>&1; then break; fi
    kill -0 "$PID" 2>/dev/null || fail "Halberd exited before its window appeared"
    sleep 1
done
xdotool search --name "Halberd Map Editor" >/dev/null 2>&1 || fail "no window after 60 seconds"

# Let the first frames draw, then take the screenshot.
sleep 3
import -window root "$OUT/window.png"

# File menu sits at the top-left; Quit is its first item.
xdotool mousemove 19 11 click 1
sleep 1
xdotool mousemove 30 36 click 1

for _ in $(seq 1 20); do
    kill -0 "$PID" 2>/dev/null || break
    sleep 1
done
kill -0 "$PID" 2>/dev/null && fail "File > Quit did not close Halberd"

wait "$PID" || fail "Halberd exited with an error"

LAYOUT_FILE="$XDG_DATA_HOME/halberd/app.ron"
grep -q "halberd_panel_layout" "$LAYOUT_FILE" 2>/dev/null ||
    fail "the panel layout was not saved to $LAYOUT_FILE"

echo "Smoke test passed: window opened, screenshot taken, File > Quit worked, layout saved."
