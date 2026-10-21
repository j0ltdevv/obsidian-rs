#!/bin/bash
# Expérience complète : Chromium + shim + moteur + harnais, avec sonde CDP,
# capture d'écran et mesure mémoire (RSS/PSS) de l'arbre de processus.
set -u
ROOT=/data/home/obsidian-rea
LOG=$ROOT/logs
DISP=${DISP:-:79}
CDP_PORT=${CDP_PORT:-9224}
HTTP_PORT=${HTTP_PORT:-8898}
URL=${URL:?URL obligatoire}
TAG=${TAG:?TAG obligatoire}
WAIT=${WAIT:-25000}
export HOME=$ROOT/home
mkdir -p "$LOG"

Xvfb "$DISP" -screen 0 1400x900x24 >"$LOG/xvfb-$TAG.log" 2>&1 &
XPID=$!
sleep 2

cd "$ROOT/asar-x1"
python3 -m http.server "$HTTP_PORT" --bind 127.0.0.1 >"$LOG/http-$TAG.log" 2>&1 &
HPID=$!
sleep 1

"$ROOT/bin/obsidian-engine" --root "$ROOT/vault" --port 27123 >"$LOG/engine-$TAG.log" 2>&1 &
EPID=$!
sleep 1

DISPLAY=$DISP chromium --user-data-dir="$LOG/chrome-$TAG" --no-sandbox --disable-gpu \
  --disable-dev-shm-usage --no-first-run --no-default-browser-check \
  --window-size=1400,900 "--remote-debugging-port=$CDP_PORT" \
  --enable-logging=stderr --v=0 "$URL" \
  >"$LOG/chrome-$TAG.stdout" 2>"$LOG/chrome-$TAG.stderr" &
CPID=$!
sleep 5

node "$ROOT/tools/cdp.mjs" "$CDP_PORT" "$WAIT" "$LOG/$TAG-page.png" \
  "@$ROOT/tools/probe.js" >"$LOG/$TAG-cdp.json" 2>"$LOG/$TAG-cdp.err"

DISPLAY=$DISP import -window root "$LOG/$TAG-screen.png" 2>>"$LOG/import.log" && echo "capture écran OK"

echo "=== mémoire (arbre chromium) ==="
bash "$ROOT/tools/mem.sh" "chromium" 0

echo "=== journal console ==="
grep -E "CONSOLE" "$LOG/chrome-$TAG.stderr" | sed 's/.*INFO:CONSOLE[^ ]* //' | head -40

kill $CPID 2>/dev/null; kill $EPID 2>/dev/null; kill $HPID 2>/dev/null; sleep 1; kill $XPID 2>/dev/null
echo "=== fin ==="
