#!/bin/bash
# mkfont.sh IDS [ROUNDS] : derive the font spec from the formulas IDS (even|odd|all), rebuild the font and the Rust table, and
#   1. add the (size, shift) pairs that the Rust encoder missed on the corpus (ROUNDS times; the committed font is `mkfont.sh all 2`);
#   2. add the glyphs that carry TeX's spacing (binary operators, relations, punctuation) and the script space after a script, and
#   3. choose the named spacers from the distances that the encoder wrote spacers for (NROUNDS times, 2).
# Environment: BROTLI (the brotli command line tool), S (scratch directory with data/, fonts/, the harness and the sandbox wrappers),
# C (crates/katex of the workspace), MINF and CLOS (a pair needed by this many formulas is added / gets the closure glyph set; 2 and 8),
# MINGAIN and MAXNAMED (a named spacer must save this many code points on the corpus, at most this many; 30 and 80).
set -e
: "${BROTLI:?mkfont.sh: set BROTLI to the brotli command line tool}"
command -v "$BROTLI" >/dev/null 2>&1 || { echo "mkfont.sh: BROTLI=$BROTLI is not an executable" >&2; exit 1; }
export BROTLI
IDS=$1; ROUNDS=${2:-1}; NROUNDS=${NROUNDS:-2}
S=${S:-/home/agents/scratch/katex-font}; C=${C:-/home/agents/workspaces/katex-rs-font/crates/katex}
cd $S
H="$S/target-ch/release/katex-font-harness"; CORPUS=/home/agents/scratch/katex-harden/formulas.bin
FONTS='Main-Regular,Math-Italic,Size1-Regular,AMS-Regular,Size2-Regular,Size3-Regular,Size4-Regular'
spec() {   # spec [EXTRA.json]: the build's spec: 64 pairs, closure for the top 16, an AMS subset, Main's ascent and descent, private-use code points only
  python3 -B -I tools/mfspec.py data spec_mk.json --fonts $FONTS --npairs 64 --closure 16 --nspacers 16 --ams-subset --ascent 0.903 --descent 0.272 --ids $IDS ${1:+--extra $1}
  python3 -B - <<'P'
import json
s = json.load(open('spec_mk.json'))
s['overlays'] = json.load(open('data/overlays.json')); s['rules'] = [[0.04, 0.23]]; s['pua_only'] = True; s['alt_letters'] = False
s['spacers'] = {}
json.dump(s, open('spec_mk.json', 'w'), separators=(',', ':'))
P
}
build() {   # build SPEC: font, Rust table, harness
  rm -rf $C/data/math-font/KaTeXMathRun-*
  python3 -B -I $C/tools/gen_math_font.py --fonts fonts --spec $1 --out $C/data/math-font --name KaTeXMathRun | cut -c1-60
  python3 -B $C/tools/gen_mathrun_table.py
  ./sxh cargo build --release --offline 2>&1 | grep -E "^error" -A6 || true
}
collect() {   # collect VAR OUTPREFIX: what the encoder wrote (MATHRUN_RUNS or MATHRUN_SPACES), inline and display
  for d in inline display; do ./sxh bash -c "$1=1 $H render $CORPUS $S/rsout/dbg_$d.jsonl font 1 $d 2> $S/$2_$d.txt"; done
  cat $2_inline.txt $2_display.txt > $2_all.txt
}
# 1. the pairs
spec; build spec_mk.json
for r in $(seq 1 $ROUNDS); do
  for d in inline display; do ./sxh bash -c "MATHRUN_MISSING=1 $H render $CORPUS $S/rsout/dbg_$d.jsonl font 1 $d 2>&1" > rsout/why_$d.txt; done
  python3 -B tools/mfpairs.py rsout/why_inline.txt rsout/why_display.txt --out extra_$r.json --ids $IDS --min ${MINF:-2} --closure ${CLOS:-8} --spec spec_mk.json
  spec extra_$r.json; build spec_mk.json
done
# 2. the glyphs that carry spacing; the script space after the script glyphs that it follows (from the runs the encoder wrote)
python3 -B - <<'P'
import json
s = json.load(open('spec_mk.json'))
s['spacing'] = {'bin': [[0.2222, 0.2222], [0.2222, 0], [0, 0.2222]], 'rel': [[0.2778, 0.2778], [0.2778, 0], [0, 0.2778]], 'punct': [[0, 0.1667]]}
json.dump(s, open('spec_mk.json', 'w'), separators=(',', ':'))
P
collect MATHRUN_RUNS runs
python3 -B tools/mfscriptspace.py script_space.json runs_all.txt
python3 -B - <<'P'
import json
s = json.load(open('spec_mk.json')); s['script_space'] = json.load(open('script_space.json'))
json.dump(s, open('spec_mk.json', 'w'), separators=(',', ':'))
P
build spec_mk.json
# 3. the named spacers
for r in $(seq 1 $NROUNDS); do
  collect MATHRUN_SPACES sp
  python3 -B tools/mfnamed.py spacers_$r.json sp_all.txt --min-gain ${MINGAIN:-30} --max ${MAXNAMED:-80} | tail -2
  python3 -B - $r <<'P'
import json, sys
s = json.load(open('spec_mk.json')); n = json.load(open('spacers_%s.json' % sys.argv[1]))
s['spacers'] = {('n%d' % -v if v < 0 else 'g%d' % v): round(v / 4000, 6) for v in n['named']}
json.dump(s, open('spec_mk.json', 'w'), separators=(',', ':'))
P
  build spec_mk.json
done
cp spec_mk.json $C/data/math-font/spec.json
for d in inline display; do ./sxh bash -c "$H render $CORPUS $S/rsout/font_$d.jsonl font 1 $d" > /dev/null 2>&1; done
python3 -B tools/mfchars.py $C/data/math-font/KaTeXMathRun-*.map.json rsout/font_inline.jsonl rsout/font_display.jsonl
