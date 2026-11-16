#!/bin/bash
# regen.sh [MINCOUNT]: regenerate the FSTRUTS and SHIFTS tables of compact.rs and the stylesheet (the other tables are frozen).
set -e
cd /home/agents/scratch/katex-compact
T=${1:-3}; EVEN=${EVEN:---even}
W=/home/agents/workspaces/katex-rs-compact
G=$W/crates/katex/tools
ALL=491519
build() { ./sxb cargo build --release --offline --manifest-path rs/Cargo.toml 2>&1 | grep -E "^(warning|error)" -A8 | head -30 || true; }
# empty tables first, so that every strut and shift is written out
python3 -I - <<P
import re
p='$W/crates/katex/src/compact.rs'
s=open(p).read()
s=re.sub(r'(// FSTRUTS-BEGIN\n).*?(// FSTRUTS-END)', r'\1const FSTRUTS: &[(&str, &str)] = &[];\n\2', s, flags=re.S)
s=re.sub(r'(// SHIFTS-BEGIN\n).*?(// SHIFTS-END)', r'\1const SHIFTS: &[[&str; 3]] = &[];\n\2', s, flags=re.S)
open(p,'w').write(s)
P
build
./sxb rs/target/release/ch render /home/agents/scratch/katex-harden/formulas.bin v2/data/gen_flat.jsonl compact:$((ALL-8-16384)):nolabel:p${P:-0} 1
python3 -I $G/gen_compact_flat.py v2/data/gen_flat.jsonl $T $EVEN
build
python3 -I $G/gen_compact_css.py /home/agents/scratch/katex-harden/npm/katex-0.18.5/package/dist/katex.css v2/katex-compact.css
cp v2/katex-compact.css $W/crates/katex/data/katex-compact.css
