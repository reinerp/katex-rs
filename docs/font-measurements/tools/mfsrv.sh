#!/bin/bash
# mfsrv.sh DIR ROUNDS REPS : server render time of the corpus, flag off (compact) and on (font), interleaved; prints per formula means of the minimum over the rounds of the per-formula median
D=$1; R=${2:-5}; N=${3:-25}; H=/home/agents/scratch/katex-font
rm -rf $D; mkdir -p $D
for r in $(seq 1 $R); do for d in inline display; do for m in compact font; do
  $H/sxh bash -c "$H/target-ch/release/katex-font-harness render /home/agents/scratch/katex-harden/formulas.bin $D/${m}_${d}_$r.jsonl $m $N $d >/dev/null 2>&1"
done; done; done
python3 - $D $R <<'P'
import json,sys,statistics
D,R=sys.argv[1],int(sys.argv[2])
for d in ('inline','display'):
    res={m:[[x['ns'] for x in map(json.loads,open('%s/%s_%s_%d.jsonl'%(D,m,d,r))) if 'html' in x] for r in range(1,R+1)] for m in ('compact','font')}
    n=len(res['compact'][0])
    a=[min(res['compact'][r][i] for r in range(R)) for i in range(n)]
    b=[min(res['font'][r][i] for r in range(R)) for i in range(n)]
    print(d,'n',n,'flat mean %.1f us, hybrid mean %.1f us, ratio of sums %.2f, median per-formula ratio %.2f'%(statistics.mean(a)/1e3,statistics.mean(b)/1e3,sum(b)/sum(a),statistics.median([y/x for x,y in zip(a,b)])), '| load', open('/proc/loadavg').read().split()[0])
P
