import sys,json
for l in open(sys.argv[1]):
    if not l.startswith('{'): continue
    j=json.loads(l); print('%-14s settled %5s %s layout %s style %s script %s nodes %s load %s'%(j['page'], j['settled_ms_median'], j['settled_ms_all'], j['layout_ms'],j['style_ms'],j['script_ms'],j['nodes'], j['loadavg'][0]))
