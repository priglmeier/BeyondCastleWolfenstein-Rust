import sys, bisect
a=open(sys.argv[1],'rb').read(); b=open(sys.argv[2],'rb').read()
syms=[]
for l in open('/home/claude/bcw/assets/symbols.txt'):
    seg,v,n=l.split()
    syms.append((seg,int(v,16),n))
def name(seg,off):
    c=[(v,n) for s,v,n in syms if s==seg and v<=off]
    if not c: return '?'
    v,n=max(c); return f'{n}+{off-v:X}'
regions=[('codeSeg',0,0x5B08),('dataSeg',0x5B08,0x7004),('vram',0x5B08+0x7004,0x4000),('cursor',0x5B08+0x7004+0x4000,2)]
diffs=[i for i in range(min(len(a),len(b))) if a[i]!=b[i]]
print(len(diffs),'differing bytes; sizes',len(a),len(b))
shown=0
for i in diffs:
    for r,s,l in regions:
        if s<=i<s+l:
            off=i-s
            nm = name(r,off) if r in('codeSeg','dataSeg') else ''
            if r=='dataSeg' and off>=0x3000: nm += f' (900:{off-0x3000:04X})'
            print(f'{r}:{off:04X} {nm}: harness={a[i]:02X} port={b[i]:02X}')
    shown+=1
    if shown>=int(sys.argv[3] if len(sys.argv)>3 else 40): break
