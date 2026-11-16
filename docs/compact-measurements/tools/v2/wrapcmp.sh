#!/bin/bash
# wrapcmp.sh NAME MODE WIDTH : line starts of the compact output (MODE) against KaTeX JS in paragraphs of WIDTH px
set -e
cd /home/agents/scratch/katex-compact
N=$1; MODE=$2; W=$3
[ -f v2/geo/wrap_js_$W.json ] || { python3 -I v2/tools/wrappage.py v2/geo/wrap_js_$W.html data/js_inline.jsonl /home/agents/scratch/katex-harden/npm/katex-0.18.5/package/dist/katex.min.css $W; ./sxc python3 -I v2/tools/wrapgeom.py v2/geo/wrap_js_$W.html v2/geo/wrap_js_$W.json; }
./sxb rs/target/release/ch render /home/agents/scratch/katex-harden/formulas.bin v2/data/w_$N.jsonl "$MODE" 1 inline
python3 -I v2/tools/wrappage.py v2/geo/wrap_${N}_$W.html v2/data/w_$N.jsonl ${CSS:-/home/agents/scratch/katex-compact/v2/katex-compact.css} $W
./sxc python3 -I v2/tools/wrapgeom.py v2/geo/wrap_${N}_$W.html v2/geo/wrap_${N}_$W.json
python3 -I - <<P
import json
a={j['id']:j for j in json.load(open('v2/geo/wrap_js_$W.json'))}; b={j['id']:j for j in json.load(open('v2/geo/wrap_${N}_$W.json'))}
wrapped=[i for i in a if a[i]['starts']]
same=[i for i in wrapped if a[i]['starts']==b[i]['starts']]
extra=[i for i in a if not a[i]['starts'] and b[i]['starts']]
hd=[i for i in a if abs(a[i]['h']-b[i]['h'])>0.05]
print('width $W: formulas that wrap in KaTeX JS: %d; same line starts: %d; different: %d (%s); wrap only in compact: %d; paragraph height differs: %d' % (len(wrapped), len(same), len(wrapped)-len(same), [i for i in wrapped if i not in same][:10], len(extra), len(hd)))
P
