#!/bin/bash
# check.sh NAME MODE [DPRS=1] : render corpus in MODE inline+display, screenshot, compare with KaTeX JS 0.18.5
set -e
NAME=$1; MODE=$2; DPRS=${3:-1}
cd /home/agents/scratch/katex-compact
B=/home/agents/scratch/katex-harden/formulas.bin
CSS=${CSS:-/home/agents/scratch/katex-compact/css/katex-compact.css}
for v in inline display; do
  V=$([ $v = inline ] && echo I || echo D)
  ./sxb rs/target/release/ch render $B data/${NAME}${V}.jsonl "$MODE" 1 $v
  python3 -I tools/gridpages.py ${NAME}${V} data/${NAME}${V}.jsonl $CSS >/dev/null
done
./sxc python3 -I tools/pix.py 9411 png $DPRS ${NAME}I ${NAME}D | tail -1
for d in ${DPRS//,/ }; do for V in I D; do node tools/pixcmp.mjs png js$V ${NAME}$V $d; done; done
