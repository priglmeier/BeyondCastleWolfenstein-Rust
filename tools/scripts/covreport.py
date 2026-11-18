import re,sys
cov=open(sys.argv[1],'rb').read()
lst=open('/home/claude/ref/bcw.lst',encoding='latin-1').read().split('\n')
cur='?'; total=0; hit=0; unc={}
for line in lst:
    if 'codeSeg ends' in line: break
    m=re.match(r'^([0-9A-F]{4}) ((?:[0-9A-F]{2,4}\*?\s?)+)\s+(.*)$', line)
    lab=re.match(r'^\s*(?:[0-9A-F]{4}\s+)?(?:[0-9A-F ]+)?\s*([A-Za-z_][\w?]*)(::?|\s+proc)', line[30:] if len(line)>30 else '')
    src=line[32:] if len(line)>32 else ''
    l2=re.match(r'\s*([A-Za-z_][\w?@]*)(::?|\s+proc\b)', src)
    if l2: cur=l2.group(1)
    if m:
        body=src.strip().lower()
        if re.match(r'^(\w+\s+)?(db|dw)\b', body) or body.startswith(('db','dw')): continue
        if not body or body.startswith(';'): continue
        off=int(m.group(1),16); total+=1
        if cov[off]: hit+=1
        else: unc.setdefault(cur,[]).append(body[:60])
print(f'{hit}/{total} instructions executed ({100*hit/total:.1f}%)')
for k,v in sorted(unc.items(), key=lambda x:-len(x[1])):
    print(f'{len(v):4d} {k}')
