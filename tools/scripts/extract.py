import struct, re, sys, os
ref = '/home/claude/ref'
out = '/home/claude/bcw/assets'
d = open(f'{ref}/bcw.exe','rb').read()
sig,last,pages,nrel,hdr,minx,maxx,ss,sp,ck,ip,cs,rel = struct.unpack('<13H', d[:26])
img = d[hdr*16:]
relocs = [struct.unpack('<HH', d[rel+4*i:rel+4*i+4]) for i in range(nrel)]
# segment sizes from listing
lst = open(f'{ref}/bcw.lst', encoding='latin-1').read()
segs = {}
for m in re.finditer(r'^(\w+) [ .]+\s+16 Bit\s+([0-9A-F]+)\s', lst, re.M):
    segs[m.group(1)] = int(m.group(2),16)
order = ['codeSeg','dataSeg','bcwLogoSeg','soundBlasterSeg','soundblasterSeg2','stackSeg']
base = {}; p = 0
for s in order:
    base[s] = p
    p += (segs[s] + 15) & ~15
print({k:hex(v) for k,v in base.items()}, 'ss', hex(ss), 'img', hex(len(img)))
assert base['stackSeg']//16 == ss, 'stack seg mismatch'
# check relocations point at segment values
for o,sg in relocs:
    a = sg*16+o
    print('reloc at', hex(a), 'value', hex(struct.unpack('<H', img[a:a+2])[0]))
names = {'codeSeg':'code','dataSeg':'data','bcwLogoSeg':'logo','soundBlasterSeg':'sb1','soundblasterSeg2':'sb2'}
for s,n in names.items():
    blob = img[base[s]:base[s]+segs[s]]
    blob = blob + bytes(segs[s]-len(blob))
    open(f'{out}/{n}.bin','wb').write(blob)
# symbols
syms = []
symsec = lst[lst.index('\nSymbols:'):]
for m in re.finditer(r'^\s*([\w?@$]+) [ .]*\s+(?:L Near|Byte(?:\[\d+\])?|Word(?:\[\d+\])?|P Near)\s+([0-9A-F]+)h?\s+(\w+)', lst, re.M):
    syms.append((m.group(1), int(m.group(2),16), m.group(3)))
print(len(syms), 'symbols')
seen = {}
for n,v,s in syms:
    seen.setdefault((n,s), v)
with open(f'{out}/symbols.txt','w') as f:
    for (n,s),v in sorted(seen.items(), key=lambda x:(x[0][1],x[1])):
        f.write(f'{s} {v:04X} {n}\n')
