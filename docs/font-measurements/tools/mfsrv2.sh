#!/bin/bash
# mfsrv2.sh DIR ROUNDS REPS : server render time of the corpus with the step on: the encoder of the commit before this stage (target-chbase2, ./sxhb2) against the new one
D=$1; R=${2:-5}; N=${3:-25}; S=/home/agents/scratch/katex-font
rm -rf $D; mkdir -p $D
for r in $(seq 1 $R); do for d in inline display; do
  $S/sxhb2 bash -c "$S/target-chbase2/release/katex-font-harness render /home/agents/scratch/katex-harden/formulas.bin $D/old_${d}_$r.jsonl font $N $d >/dev/null 2>&1"
  $S/sxh bash -c "$S/target-ch/release/katex-font-harness render /home/agents/scratch/katex-harden/formulas.bin $D/new_${d}_$r.jsonl font $N $d >/dev/null 2>&1"
  $S/sxh bash -c "$S/target-ch/release/katex-font-harness render /home/agents/scratch/katex-harden/formulas.bin $D/flat_${d}_$r.jsonl compact $N $d >/dev/null 2>&1"
done; done
python3 - $D $R <<'P'
import json,sys,statistics
D,R=sys.argv[1],int(sys.argv[2])
for d in ('inline','display'):
    res={m:[[x['ns'] for x in map(json.loads,open('%s/%s_%s_%d.jsonl'%(D,m,d,r))) if 'html' in x] for r in range(1,R+1)] for m in ('old','new','flat')}
    n=len(res['old'][0]); mn={m:[min(res[m][r][i] for r in range(R)) for i in range(n)] for m in res}
    print(d,'n',n,'flat mean %.1f us, old hybrid %.1f us, new hybrid %.1f us; new/old %.3f (median per formula %.3f), new/flat %.3f'%(statistics.mean(mn['flat'])/1e3,statistics.mean(mn['old'])/1e3,statistics.mean(mn['new'])/1e3,sum(mn['new'])/sum(mn['old']),statistics.median([y/x for x,y in zip(mn['old'],mn['new'])]),sum(mn['new'])/sum(mn['flat'])),'| load',open('/proc/loadavg').read().split()[0])
P
