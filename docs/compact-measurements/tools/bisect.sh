#!/bin/bash
# bisect.sh NAME LEVELS [P] : render inline+display at dpr 2 on the first MAXP pages, compare with KaTeX JS
cd /home/agents/scratch/katex-compact
N=$1; M=compact:$2:nolabel:p${3:-0}
for v in inline display; do V=$([ $v = inline ] && echo I || echo D)
  ./sxb rs/target/release/ch render /home/agents/scratch/katex-harden/formulas.bin data/${N}${V}.jsonl "$M" 1 $v
  python3 -I tools/gridpages.py ${N}${V} data/${N}${V}.jsonl css/katex-compact.css >/dev/null; done
MAXP=${MAXP:-12} ./sxc python3 -I tools/pix.py 9411 png 2 ${N}I ${N}D | tail -1
for V in I D; do MAXP=${MAXP:-12} node tools/pixcmp.mjs png js$V ${N}$V 2 | head -2; done
