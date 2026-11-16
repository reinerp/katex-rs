#!/bin/bash
# The sequence behind crates/katex/data/math-font/ and the numbers in docs/FONT.md. Paths are those of the machine the
# measurements were made on (scratch directory ~/scratch/katex-font with the KaTeX 0.18.5 TTFs in fonts/, the corpus
# in ~/scratch/katex-harden/data/formulas.jsonl, the sandbox wrapper ./sxc for Chrome); edit them for another one.
# The compiler itself needs only python3 and (for WOFF 2) the `brotli` command line tool.
set -e
S=~/scratch/katex-font
T=$(dirname "$0")/tools
C=$(dirname "$0")/../../crates/katex
export BROTLI=$S/brotli

# 1. per-glyph geometry of every corpus formula from KaTeX JS 0.18.5 (Chrome), inline and display
cd $S && ./sxc python3 $T/mfextract.py data
# 2. coverage of (size, shift) pairs, glyph sets, gap clusters (the named spacers), accents
python3 -I $T/mfanalyze.py data inline
./sxc python3 $T/mfaccents.py data/overlays.json
# 3. the recommended build: Main-Regular, Math-Italic, Size1-4, AMS (subset), top 64 (size, shift) pairs, accents, rules
python3 -I $T/mfspec.py data spec_rec.json --fonts 'Main-Regular,Math-Italic,Size1-Regular,AMS-Regular,Size2-Regular,Size3-Regular,Size4-Regular' \
        --npairs 64 --closure 16 --nspacers 16 --ams-subset
python3 - <<'E'
import json
s = json.load(open('spec_rec.json'))
s['overlays'] = json.load(open('data/overlays.json')); s['rules'] = [[0.04, 0.23]]
json.dump(s, open('spec_rec.json', 'w'), separators=(',', ':'))
E
python3 -I $C/tools/gen_math_font.py --fonts fonts --spec spec_rec.json --out $C/data/math-font --name KaTeXMathRun
cp spec_rec.json $C/data/math-font/spec.json
# 4. checks: every format loads and renders the same, fidelity against KaTeX JS, shaping, context, benchmarks
./sxc python3 $T/fmtcheck.py $C/data/math-font/KaTeXMathRun
./sxc python3 $T/mftest.py $C/data/math-font/KaTeXMathRun results/fid --ids all --dpr 1,2 && python3 -I $T/mfpix.py results/fid 80
./sxc python3 $T/mfshape.py $C/data/math-font/KaTeXMathRun results/shape
./sxc python3 $T/mfctx.py results/ctx
python3 -I $T/mfsizes.py results/sizes
python3 -I $T/mfhybrid.py $C/data/math-font/KaTeXMathRun bench/hybrid.jsonl     # then mkpage.py and measure3.py, see results/README
