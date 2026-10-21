#!/bin/bash
# Mesure de la consommation memoire d'un arbre de processus (RSS + PSS).
# Usage : mem.sh <nom d executable> [delai avant mesure en secondes]
set -u
NAME=${1:?nom d executable obligatoire}
WAIT=${2:-0}
[ "$WAIT" != "0" ] && sleep "$WAIT"

total_rss=0
total_pss=0
count=0
echo "=== processus $NAME ==="
for pid in $(ps -eo pid=,comm= | awk -v n="$NAME" "\$2==n {print \$1}"); do
  rss=$(awk "/^VmRSS:/{print \$2; exit}" "/proc/$pid/status" 2>/dev/null)
  pss=$(awk "/^Pss:/{print \$2; exit}" "/proc/$pid/smaps_rollup" 2>/dev/null)
  [ -z "${rss:-}" ] && continue
  args=$(ps -o args= -p "$pid" | cut -c1-55)
  echo "  pid=$pid rss=${rss}K pss=${pss:-?}K $args"
  total_rss=$((total_rss + rss))
  total_pss=$((total_pss + ${pss:-0}))
  count=$((count + 1))
done
echo "TOTAL ($count processus) rss=$((total_rss / 1024)) M pss=$((total_pss / 1024)) M"
