#!/bin/bash
# geo.sh NAME LEVELS|js [CSS]: per-glyph geometry of the whole corpus (inline and display, plain rows and in a paragraph) -> v2/geo/NAMEI.json ...
set -e
cd /home/agents/scratch/katex-compact
NAME=$1; MODE=$2; CSS=${3:-/home/agents/scratch/katex-compact/v2/katex-compact.css}
B=/home/agents/scratch/katex-harden/formulas.bin
for v in inline display; do
  V=$([ $v = inline ] && echo I || echo D)
  if [ "$MODE" = js ]; then D=data/js_$v.jsonl; C=${JSCSS:-/home/agents/scratch/katex-harden/npm/katex-0.18.5/package/dist/katex.min.css}
  else ./sxb rs/target/release/ch render $B v2/data/$NAME$V.jsonl "$MODE" 1 $v; D=v2/data/$NAME$V.jsonl; C=$CSS; fi
  python3 -I v2/tools/geompage.py v2/geo/$NAME$V.html $D $C
  python3 -I v2/tools/geompage.py v2/geo/${NAME}${V}_ctx.html $D $C --context
  ./sxc python3 -I tools/geom.py v2/geo/$NAME$V.html v2/geo/$NAME$V.json >/dev/null
  ./sxc python3 -I tools/geom.py v2/geo/${NAME}${V}_ctx.html v2/geo/${NAME}${V}_ctx.json ctx >/dev/null
done
