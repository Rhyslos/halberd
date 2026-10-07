#!/usr/bin/env bash
# Window smoke test: opens the real Halberd window on a virtual screen,
# takes a screenshot, quits with Ctrl+Q (or File > Quit as a fallback), and
# checks that the panel layout was saved.
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
    # On GitHub, also raise an annotation so the reason and the end of
    # Halberd's output show on the pull request page.
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        local msg
        msg="$1"$'\n'"Last lines of Halberd's output:"$'\n'"$(tail -n 25 "$OUT/halberd.log" 2>/dev/null)"
        msg="${msg//'%'/'%25'}"
        msg="${msg//$'\r'/'%0D'}"
        msg="${msg//$'\n'/'%0A'}"
        echo "::error title=Window smoke test::$msg"
    fi
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

# Let the first frames draw (software rendering can be slow), then take
# the screenshot.
sleep 8
import -window root "$OUT/window.png"

# Quit the way a user would. Try Ctrl+Q a few times (the pointer is moved
# over the window so it has keyboard focus), then fall back to clicking
# File > Quit in the menu at the top-left.
WINDOW_ID="$(xdotool search --name "Halberd Map Editor" | head -1)"
quit_attempt() {
    local attempt=$1
    xdotool mousemove 800 450
    # A virtual screen has no window manager to give the window keyboard
    # focus, so give it focus directly.
    xdotool windowfocus --sync "$WINDOW_ID" 2>/dev/null || true
    sleep 0.5
    if [ "$attempt" -le 3 ]; then
        echo "Quit attempt $attempt: Ctrl+Q"
        xdotool key ctrl+q
    else
        echo "Quit attempt $attempt: File > Quit"
        xdotool mousemove 19 11 click 1
        sleep 2
        xdotool mousemove 40 36 click 1
    fi
}

for attempt in 1 2 3 4 5 6; do
    quit_attempt "$attempt"
    for _ in $(seq 1 10); do
        kill -0 "$PID" 2>/dev/null || break 2
        sleep 1
    done
done
if kill -0 "$PID" 2>/dev/null; then
    import -window root "$OUT/quit-failed.png" || true
    fail "neither Ctrl+Q nor File > Quit closed Halberd (see quit-failed.png)"
fi

wait "$PID" || fail "Halberd exited with an error"

LAYOUT_FILE="$XDG_DATA_HOME/halberd/app.ron"
grep -q "halberd_panel_layout" "$LAYOUT_FILE" 2>/dev/null ||
    fail "the panel layout was not saved to $LAYOUT_FILE"

echo "Smoke test passed: window opened, screenshot taken, quit worked, layout saved."
