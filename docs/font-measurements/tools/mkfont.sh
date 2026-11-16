#!/bin/bash
# mkfont.sh IDS [ROUNDS] : derive the font spec from the formulas IDS (even|odd|all), rebuild the font and the Rust table, and add the
# (size, shift) pairs that the Rust encoder missed on the corpus (ROUNDS times; the committed font is `mkfont.sh all 2`).
# Environment: BROTLI (the brotli command line tool), S (scratch directory with data/, fonts/, the harness and the sandbox wrappers),
# C (crates/katex of the workspace), MINF and CLOS (a pair needed by this many formulas is added / gets the closure glyph set; 2 and 8).
set -e
: "${BROTLI:?mkfont.sh: set BROTLI to the brotli command line tool}"
command -v "$BROTLI" >/dev/null 2>&1 || { echo "mkfont.sh: BROTLI=$BROTLI is not an executable" >&2; exit 1; }
export BROTLI
IDS=$1; ROUNDS=${2:-1}
S=${S:-/home/agents/scratch/katex-font}; C=${C:-/home/agents/workspaces/katex-rs-font/crates/katex}
cd $S
FONTS='Main-Regular,Math-Italic,Size1-Regular,AMS-Regular,Size2-Regular,Size3-Regular,Size4-Regular'
spec() {   # spec [EXTRA.json]: the build's spec: 64 pairs, closure for the top 16, 16 named spacers, an AMS subset, Main's ascent and descent, private-use code points only
  python3 -B -I tools/mfspec.py data spec_mk.json --fonts $FONTS --npairs 64 --closure 16 --nspacers 16 --ams-subset --ascent 0.903 --descent 0.272 --ids $IDS ${1:+--extra $1}
  python3 -B - <<'P'
import json
s = json.load(open('spec_mk.json'))
s['overlays'] = json.load(open('data/overlays.json')); s['rules'] = [[0.04, 0.23]]; s['pua_only'] = True; s['alt_letters'] = False
json.dump(s, open('spec_mk.json', 'w'), separators=(',', ':'))
P
}
build() {   # font, Rust table, harness
  rm -rf $C/data/math-font/KaTeXMathRun-*
  python3 -B -I $C/tools/gen_math_font.py --fonts fonts --spec spec_mk.json --out $C/data/math-font --name KaTeXMathRun | cut -c1-60
  python3 -B $C/tools/gen_mathrun_table.py
  ./sxh cargo build --release --offline 2>&1 | grep -E "^error" -A6 || true
}
spec; build
for r in $(seq 1 $ROUNDS); do
  for d in inline display; do ./sxh bash -c "MATHRUN_MISSING=1 $S/target-ch/release/katex-font-harness render /home/agents/scratch/katex-harden/formulas.bin $S/rsout/dbg_$d.jsonl font 1 $d 2>&1" > rsout/why_$d.txt; done
  python3 -B tools/mfpairs.py rsout/why_inline.txt rsout/why_display.txt --out extra_$r.json --ids $IDS --min ${MINF:-2} --closure ${CLOS:-8} --spec spec_mk.json
  spec extra_$r.json; build
done
cp spec_mk.json $C/data/math-font/spec.json
for d in inline display; do ./sxh bash -c "$S/target-ch/release/katex-font-harness render /home/agents/scratch/katex-harden/formulas.bin $S/rsout/font_$d.jsonl font 1 $d" > /dev/null 2>&1; done
