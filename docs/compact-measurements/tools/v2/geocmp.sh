#!/bin/bash
# geocmp.sh NAME : compare against the js reference
cd /home/agents/scratch/katex-compact
for V in I D; do echo "== $NAME $V"; python3 -I v2/tools/geocmp2.py v2/geo/${REF:-js}$V.json v2/geo/$1$V.json; echo "-- context"; python3 -I v2/tools/geocmp2.py v2/geo/${REF:-js}${V}_ctx.json v2/geo/$1${V}_ctx.json ctx; done
