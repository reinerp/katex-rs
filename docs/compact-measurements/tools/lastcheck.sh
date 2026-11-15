#!/bin/bash
cd /home/agents/scratch/katex-compact
./check.sh lx compact:22047:nolabel 1,2 2>&1 | grep -v "^done" > data/last_exact.txt
./check.sh la compact::nolabel 1,2 2>&1 | grep -v "^done" > data/last_all.txt
./check.sh ll compact 1 2>&1 | grep -v "^done" > data/last_label.txt
echo done > data/last.done
