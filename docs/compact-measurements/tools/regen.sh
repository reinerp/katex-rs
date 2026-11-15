#!/bin/bash
# regen.sh [MINCOUNT] : regenerate the generated tables in compact.rs (struts, then values and groups) and the stylesheet, rebuild.
set -e
cd /home/agents/scratch/katex-compact
T=${1:-3}; EVEN=${EVEN:---even}
G=/home/agents/workspaces/katex-rs-compact/crates/katex/tools
ALL=32767
build() { ./sxb cargo build --release --offline --manifest-path rs/Cargo.toml 2>&1 | grep -E "^(warning|error)" -A8 | head -30 || true; }
./sxb rs/target/release/ch render /home/agents/scratch/katex-harden/formulas.bin data/gen_a.jsonl compact:$((ALL-8-1024-4096-16384)):nolabel:p${P:-0} 1
python3 -I $G/gen_compact_values.py data/gen_a.jsonl $T $EVEN
build
./sxb rs/target/release/ch render /home/agents/scratch/katex-harden/formulas.bin data/gen_b.jsonl compact:$((ALL-8-16384)):nolabel:p${P:-0} 1
python3 -I $G/gen_compact_values.py data/gen_b.jsonl $T $EVEN --values-only --groups
build
python3 -I $G/gen_compact_css.py /home/agents/scratch/katex-harden/npm/katex-0.18.5/package/dist/katex.css css/katex-compact.css
