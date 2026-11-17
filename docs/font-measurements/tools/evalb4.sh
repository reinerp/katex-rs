#!/bin/bash
cd /home/agents/scratch/katex-font
rm -f evalb4.done; : > bench4/res_micro.jsonl
for i in 0 1 2 3 4 5 6 7; do
  ./sxc python3 tools/measure3.py $((9810+i)) 9 bench4/m${i}_floor.html bench4/m${i}_flat.html bench4/m${i}_old.html bench4/m${i}_new.html 2>/dev/null >> bench4/res_micro.jsonl
done
./sxc python3 tools/measure3.py 9830 9 bench4/fx_floor.html bench4/fx_flat.html bench4/fx_old.html bench4/fx_new.html 2>/dev/null > bench4/res_fx.jsonl
echo done > evalb4.done
