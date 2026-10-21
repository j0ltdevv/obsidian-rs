#!/bin/bash
# Expérience F : renderer réel d'Obsidian dans une BrowserWindow Electron
# pilotée par notre harnais de processus principal (code propre).
set -u
ROOT=/data/home/obsidian-rea
LOG=$ROOT/logs
DISP=${DISP:-:81}
CDP_PORT=${CDP_PORT:-9226}
export HOME=$ROOT/home
mkdir -p "$LOG"

Xvfb "$DISP" -screen 0 1400x900x24 >"$LOG/xvfb-harness.log" 2>&1 &
XPID=$!
sleep 2

DISPLAY=$DISP "$ROOT/tools/electron/node_modules/.bin/electron" \
  "$ROOT/tools/main-harness.js" --no-sandbox --disable-gpu \
  "--remote-debugging-port=$CDP_PORT" \
  >"$LOG/electron-harness.stdout" 2>"$LOG/electron-harness.stderr" &
EPID=$!
sleep 5

node "$ROOT/tools/cdp.mjs" "$CDP_PORT" "${WAIT:-20000}" "$LOG/electron-harness-page.png" \
  "@$ROOT/tools/probe.js" >"$LOG/electron-harness-cdp.json" 2>"$LOG/electron-harness-cdp.err"

DISPLAY=$DISP import -window root "$LOG/electron-harness-screen.png" 2>>"$LOG/import.log" && echo "capture écran OK"

echo "=== mémoire (arbre electron) ==="
bash "$ROOT/tools/mem.sh" electron 0

echo "=== stdout harnais ==="
head -60 "$LOG/electron-harness.stdout"
echo "=== stderr (fin) ==="
tail -20 "$LOG/electron-harness.stderr"

kill $EPID 2>/dev/null; sleep 2; kill $XPID 2>/dev/null
echo "=== fin ==="
