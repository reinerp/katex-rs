#!/bin/bash
set -e
cd /home/agents/scratch/katex-compact
export P=$1
./regen.sh 3 > data/regen_p$P.log 2>&1
cp css/katex-compact.css css/katex-compact-p$P.css
cp /home/agents/workspaces/katex-rs-compact/crates/katex/src/compact.rs data/compact_p$P.rs
python3 -I tools/sizes.py compact::nolabel:p$P data/precsize_p$P.json
