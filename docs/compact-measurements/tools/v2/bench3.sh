#!/bin/bash
# bench3.sh RUNS page.html... : settle wall (median/min) and main thread CPU ms of the phases (median)
cd /home/agents/scratch/katex-compact
R=$1; shift
./sxc python3 -I tools/measure3.py ${PORT:-9633} $R "$@" 2>&1 | python3 -c "
import sys,json
for l in sys.stdin:
    if not l.startswith('{'): print(l.strip()); continue
    j=json.loads(l); c=lambda k: j[k+'_cpu']
    print('%-14s settle med %5d min %5d (load %.1f) | cpu: parse %4d style %4d layout %4d prepaint %4d paint %3d total-task %5d (min %5d)'%(j['page'],j['settle_med'],j['settle_min'],j['load'],c('ParseHTML'),c('UpdateLayoutTree'),c('Layout'),c('PrePaint'),c('Paint'),c('RunTask'),j['RunTask_cpumin']))
"
