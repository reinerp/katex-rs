#!/bin/bash
# pixcheck.sh NAME MODE [DPRS=1,2] [CSS] : pixel comparison of the whole corpus (inline and display) against KaTeX JS
set -e
cd /home/agents/scratch/katex-compact
NAME=$1; MODE=$2; DPRS=${3:-1,2}; CSS=${4:-/home/agents/scratch/katex-compact/v2/katex-compact.css}
B=/home/agents/scratch/katex-harden/formulas.bin
JSC=/home/agents/scratch/katex-harden/npm/katex-0.18.5/package/dist/katex.min.css
if [ ! -f v2/png/rjsD_dpr2_000.png ]; then
  python3 -I v2/tools/gridpages.py rjsI data/js_inline.jsonl $JSC >/dev/null
  python3 -I v2/tools/gridpages.py rjsD data/js_display.jsonl $JSC >/dev/null
  ./sxc python3 -I tools/pix.py 9411 v2/png $DPRS rjsI rjsD | tail -1
fi
for v in inline display; do
  V=$([ $v = inline ] && echo I || echo D)
  ./sxb rs/target/release/ch render $B v2/data/${NAME}${V}.jsonl "$MODE" 1 $v
  python3 -I v2/tools/gridpages.py ${NAME}${V} v2/data/${NAME}${V}.jsonl $CSS >/dev/null
done
./sxc python3 -I tools/pix.py 9411 v2/png $DPRS ${NAME}I ${NAME}D | tail -1
for d in ${DPRS//,/ }; do for V in I D; do echo "scale $d $V:"; node tools/pixcmp.mjs v2/png rjs$V ${NAME}$V $d; done; done
