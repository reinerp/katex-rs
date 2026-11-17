#!/bin/bash
# The sequence behind crates/katex/data/math-font/ and the numbers in docs/FONT.md. Paths are those of the machine the
# measurements were made on (scratch directory ~/scratch/katex-font with the KaTeX 0.18.5 TTFs in fonts/, the corpus in
# ~/scratch/katex-harden/{data/formulas.jsonl,formulas.bin}, the sandbox wrappers ./sxc (Chrome, python), ./sxh (cargo for the
# harness crate rs/ch = tools/ch_font.rs + ch_font-Cargo.toml), ./sxr (cargo in the workspace); edit them for another one.
# It takes about an hour. The font compiler itself needs only python3 and (for the WOFF 2) the `brotli` command line tool.
set -e
S=${S:-$HOME/scratch/katex-font}
T=$(cd "$(dirname "$0")" && pwd)/tools
C=$(cd "$(dirname "$0")/../.." && pwd)/crates/katex
# gen_math_font.py compresses the WOFF 2 with the `brotli` command line tool (build time only; the generated .woff2 is checked in)
if [ -z "${BROTLI:-}" ] || ! command -v "$BROTLI" >/dev/null 2>&1; then
    echo "regen.sh: set BROTLI to the brotli command line tool (Google's reference CLI; on Ubuntu/Debian the package 'brotli')." >&2
    echo "          it is needed to write the WOFF 2 file; nothing else here uses it." >&2
    exit 1
fi
export BROTLI S C
cd "$S"

# 1. per-glyph geometry of every corpus formula from KaTeX JS 0.18.5 (Chrome), inline and display; accent overlays; statistics
./sxc python3 $T/mfextract.py data
./sxc python3 $T/mfaccents.py data/overlays.json
python3 -B $T/mfanalyze.py data inline                    # coverage of (size, shift) pairs, glyph sets, gap clusters
# 2. the font: spec from the corpus (64 pairs, closure for the top 16, 16 named spacers, AMS subset), the font, the Rust table,
#    and two rounds of "which (size, shift) pairs did the Rust encoder need and the font lack". Writes
#    crates/katex/data/math-font/ and src/generated/mathrun_table.rs, and the renderings rsout/font_{inline,display}.jsonl.
#    `mkfont.sh even 2` is the held-out check (the font from the even-numbered formulas): evaluate it with `mfshare.py ... odd`.
$T/mkfont.sh all 2
# 3. the renderings that the checks compare with: the flat design (flag off)
for d in inline display; do
    ./sxh bash -c "$S/target-ch/release/katex-font-harness render ~/scratch/katex-harden/formulas.bin $S/rsout/compact_$d.jsonl compact 1 $d"
done
# 4. checks: containers, what is in text nodes, geometry and pixels against KaTeX JS, line breaks, screenshots
./sxc python3 $T/fmtcheck.py $C/data/math-font/KaTeXMathRun-*
python3 -B $T/mfshare.py rsout/font_inline.jsonl inline all; python3 -B $T/mfshare.py rsout/font_display.jsonl display all
python3 -B $T/mfstock.py rsout/font_inline.jsonl inline
for d in inline display; do
    ./sxc python3 $T/mfhyb.py rsout/font_$d.jsonl $d hyb_$d --dpr 1,2
    python3 -B $T/mfhpix.py hyb_$d 80                     # differing pixels per formula
    python3 -B $T/mfshift.py hyb_$d 80                    # formulas that match better one device pixel shifted
    for r in 1 2; do python3 -B $T/mfcent.py hyb_$d 80 $r; done   # ink centroids
done
for w in 300 760; do                                      # same line starts and paragraph heights as KaTeX JS
    ./sxc python3 $T/mfwrap.py rsout/font_inline.jsonl wrapg_h$w $w                # against KaTeX in geometricPrecision
    ./sxc python3 $T/mfwrap.py rsout/font_inline.jsonl wrap_h$w $w ktauto          # against KaTeX as served (auto)
    ./sxc python3 $T/mfwrap.py rsout/compact_inline.jsonl wrap_f$w $w ktauto flat  # the flat design, for reference
done
./sxc python3 $T/mfshow.py show1                          # side-by-side screenshot pages, dpr 1 and 2
./sxc python3 $T/neg_spacer_across_elements.py            # negative spacers next to elements
# 5. speed: pages (floor / flat / hybrid) and the interleaved runs (evalb.sh: micro formulas and the fixture), the server
./sxc python3 $T/mfbench3.py && $T/evalb.sh
$T/mfsrv.sh $S/bench_srv 5 25
# 5b. the second hybrid stage (spacing in the glyphs, line breaks as characters) against the output of the commit before it: build that commit
#     (a jj workspace at 583ecb09 and the harness crate against it: tools/ch_font.rs without the collectors), render the corpus with it into old/font_{inline,display}.jsonl,
#     copy its font and stylesheet to old/data/ and rsout/font_old.css, then
#       ./sxc python3 $T/mfpos.py old/font_inline.jsonl old/data/KaTeXMathRun-<old>.map.json old/data/KaTeXMathRun-<old>.woff2 rsout/font_old.css \
#               rsout/font_inline.jsonl $C/data/math-font/KaTeXMathRun-*.map.json $C/data/math-font/KaTeXMathRun-*.woff2 rsout/font.css inline posF_inline
#       ./sxc python3 $T/mfold.py old/font_inline.jsonl rsout/font_old.css old/data/KaTeXMathRun-<old>.woff2 rsout/font_inline.jsonl rsout/font.css \
#               $C/data/math-font/KaTeXMathRun-*.woff2 inline pixF_inline
#       ./sxc python3 $T/mfwrap.py rsout/font_inline.jsonl wrapF_300 300; ... custom=$T/targeted.tex ...
#     micro and fixture pages: python3 $T/mfbench4.py bench4 && $T/evalb4.sh; server: $T/mfsrv2.sh $S/bench_srv2 5 25
# 6. tests
./sxr cargo test --offline
