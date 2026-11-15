#!/bin/bash
cd /home/agents/scratch/katex-compact
for P in 4 3 2; do
  ./check.sh q$P compact::nolabel:p$P 1,2 2>&1 | grep -v "^done" > data/prec_pix_p$P.txt
done
# the same with the vertical extents rounded too
for P in 3; do
  ./check.sh r$P compact:65535:nolabel:p$P 1,2 2>&1 | grep -v "^done" > data/prec_pix_ext_p$P.txt
done
echo done > data/prec.done
