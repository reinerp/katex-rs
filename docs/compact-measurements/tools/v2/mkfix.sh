#!/bin/bash
# mkfix.sh NAME LEVELS CSS : fixture page v2/pages/fx_NAME.html from the corpus (original display flags)
set -e
cd /home/agents/scratch/katex-compact
./sxb rs/target/release/ch render /home/agents/scratch/katex-harden/formulas.bin v2/data/fx_$1.jsonl "$2" 1
python3 -I tools/mkpage.py v2/pages/fx_$1.html v2/data/fx_$1.jsonl --css $3
