# repeat.py DATA.jsonl [fixture] : how much of the markup is formulas / subtrees seen before
import json, re, sys, collections
rows = [json.loads(l) for l in open(sys.argv[1])]
ids = [r['id'] for r in rows if 'html' in r]
htm = {r['id']: r['html'] for r in rows if 'html' in r}
seq = [ids[k % len(ids)] for k in range(4800)] if len(sys.argv) > 2 else ids
tok = re.compile(r'<(/?)(span|svg|path|line|a|img)\b[^>]*?(/?)>')
def subtrees(h):
    out = []; st = []
    for m in tok.finditer(h):
        close, tag, selfc = m.group(1), m.group(2), m.group(3)
        if selfc: out.append((m.start(), m.end())); continue
        if not close: st.append(m.start())
        else:
            s = st.pop(); out.append((s, m.end()))
    return out
# whole-formula repeats
seen = set(); rep_bytes = 0; tot = 0
for i in seq:
    tot += len(htm[i])
    if i in seen: rep_bytes += len(htm[i])
    seen.add(i)
print('formula occurrences %d unique %d; bytes in repeated formulas %.1f%%' % (len(seq), len(set(seq)), 100 * rep_bytes / tot))
# subtree repeats among unique formulas (first occurrence order)
cnt = collections.Counter(); allsub = {}
for i in ids:
    h = htm[i]; subs = subtrees(h); allsub[i] = [(h[a:b]) for a, b in subs]
    cnt.update(allsub[i])
elems = sum(len(v) for v in allsub.values())
dup_el = sum(1 for v in allsub.values() for s in v if cnt[s] >= 2)
# maximal repeated subtrees: bytes of 2nd+ occurrences not inside a bigger repeated subtree
seenmax = set(); dedup_bytes = 0; total_bytes = 0
for i in ids:
    h = htm[i]; total_bytes += len(h); covered = []
    for a, b in sorted(subtrees(h), key=lambda t: (t[0], -t[1])):
        if covered and a < covered[-1]: continue
        s = h[a:b]
        if cnt[s] >= 2 and len(s) >= 40:
            if s in seenmax: dedup_bytes += len(s)
            seenmax.add(s); covered.append(b)
print('unique formulas: elements %d, in subtrees that occur at least twice %.1f%%; bytes of later copies of maximal repeated subtrees (>=40 B) %.1f%% of all bytes' % (elems, 100 * dup_el / elems, 100 * dedup_bytes / total_bytes))
