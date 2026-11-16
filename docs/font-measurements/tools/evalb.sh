#!/bin/bash
cd /home/agents/scratch/katex-font
rm -f evalb.done; : > bench3/res_micro.jsonl
for i in 0 1 2 3 4 5 6 7 8 9; do
  ./sxc python3 tools/measure3.py $((9710+i)) 7 bench3/m${i}_floor.html bench3/m${i}_flat.html bench3/m${i}_hyb.html 2>/dev/null >> bench3/res_micro.jsonl
done
./sxc python3 tools/measure3.py 9730 9 bench3/fx_floor.html bench3/fx_flat.html bench3/fx_hyb.html 2>/dev/null > bench3/res_fx.jsonl
echo done > evalb.done
