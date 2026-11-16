#!/bin/bash
cd /home/agents/scratch/katex-font
rm -f evalw.done
for w in 300 760; do
  ./sxc python3 tools/mfwrap.py rsout/font_inline.jsonl wrapg_h$w $w > wrapg_h$w.log 2>&1
  ./sxc python3 tools/mfwrap.py rsout/font_inline.jsonl wrap_h$w $w ktauto > wrap_h$w.log 2>&1
  ./sxc python3 tools/mfwrap.py rsout/compact_inline.jsonl wrap_f$w $w ktauto flat > wrap_f$w.log 2>&1
done
echo done > evalw.done
