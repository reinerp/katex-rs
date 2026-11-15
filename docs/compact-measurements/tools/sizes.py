# sizes.py MODE OUT.json : render the corpus in MODE and print/store size stats (corpus, held-out odd ids, fixture)
import json, sys, subprocess, re
sys.path.insert(0, '/home/agents/scratch/katex-compact/tools')
from levels import stats, D, BIN
mode, out = sys.argv[1:3]
fn = D + 'data/sz_%s.jsonl' % re.sub(r'\W+', '_', mode)
subprocess.run([D + 'sxb', D + 'rs/target/release/ch', 'render', BIN, fn, mode, '1'], check=True)
rows = {j['id']: j for j in map(json.loads, open(fn))}
ids = sorted(i for i, j in rows.items() if 'html' in j)
res = dict(mode=mode,
           corpus=stats([rows[i]['html'] for i in ids]),
           heldout=stats([rows[i]['html'] for i in ids if i % 2]),
           fixture=stats([rows[ids[k % len(ids)]]['html'] for k in range(4800)]))
json.dump(res, open(out, 'w'))
c = res['corpus']; h = res['heldout']
print('%-28s corpus raw %7d zstd %6d gz %6d el %6d | heldout raw %7d zstd %6d gz %6d el %6d' % (mode, c['raw'], c['zstd'], c['gzip'], c['el'], h['raw'], h['zstd'], h['gzip'], h['el']))
