#!/bin/bash
# rsrun.sh MODE DISPLAY 'tex'... : render the given formulas with the Rust harness, print the html with private-use characters shown as <U+XXXX>
python3 - "$@" <<'P'
import sys, subprocess, json, re
mode, disp, texs = sys.argv[1], sys.argv[2], sys.argv[3:]
b = ''.join('%d %d %d\n%s\n' % (i, 0, len(t.encode()), t) for i, t in enumerate(texs))
open('/home/agents/scratch/katex-font/rsout/one.bin', 'w').write(b)
r = subprocess.run(['/home/agents/scratch/katex-font/sxh', 'bash', '-c', 'MATHRUN_DEBUG=1 /home/agents/scratch/katex-font/target-ch/release/katex-font-harness render /home/agents/scratch/katex-font/rsout/one.bin /home/agents/scratch/katex-font/rsout/one.jsonl %s 1 %s' % (mode, disp)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
print(r.stderr[-1500:])
for l in open('/home/agents/scratch/katex-font/rsout/one.jsonl'):
    j = json.loads(l); h = j.get('html', j)
    print(re.sub(r'[-\U000f0000-\U000ffffd]', lambda m: '<%X>' % ord(m.group()), h) if isinstance(h, str) else h)
P
