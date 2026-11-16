#!/bin/bash
cd /home/agents/scratch/katex-font
rm -f eval.done
./sxc python3 tools/mfhyb.py rsout/font_inline.jsonl inline hyb_i --dpr 1,2 > hyb_i.log 2>&1
./sxc python3 tools/mfhyb.py rsout/font_display.jsonl display hyb_d --dpr 1,2 > hyb_d.log 2>&1
for w in 300 760; do
  ./sxc python3 tools/mfwrap.py rsout/font_inline.jsonl wrap_h$w $w > wrap_h$w.log 2>&1
  ./sxc python3 tools/mfwrap.py rsout/compact_inline.jsonl wrap_f$w $w ktauto > wrap_f$w.log 2>&1
done
echo done > eval.done
