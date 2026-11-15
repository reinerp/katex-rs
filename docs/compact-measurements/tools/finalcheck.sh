#!/bin/bash
cd /home/agents/scratch/katex-compact
P=0 ./regen.sh 3 > data/regen_final.log 2>&1
./sxb cargo build --release --offline --manifest-path rs/Cargo.toml 2>&1 | tail -1
./check.sh fx compact:22047:nolabel 1,2 2>&1 | grep -v "^done" > data/final_exact.txt
./check.sh fa compact::nolabel 1,2 2>&1 | grep -v "^done" > data/final_all.txt
echo finished > data/final.done
