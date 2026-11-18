import random, subprocess, sys, os, concurrent.futures as cf
H='/home/claude/tools/harness/target/release/harness'
P='/home/claude/bcw/target/release/bcw'
keys=list(b'qwedcxzasiop;/.,k rbf:hlum0123456789')+[13]*3+[ord('l')]*6+[ord(' ')]*6+list(b'qwedcxza')*4+[ord('u')]*3+[0x14,0x0b]*2+list(b'12345m')
def run(seed, polls, nosb, esc):
    r=random.Random(seed)
    d=f'/tmp/claude-0/fz/{seed}{"n" if nosb else ""}'
    os.makedirs(d,exist_ok=True)
    subprocess.run(['cp','/tmp/claude-0/d1/optiona_synth.dat',d+'/optiona.dat'])
    for f in ('savegame.bcw',):
        try: os.remove(f'{d}/{f}')
        except: pass
    lines=['0 000d', f'{r.randint(0,5)} 00{ord(r.choice("abb")):02x}', '2 000d']
    for _ in range(12000):
        k=r.choice(keys)
        if esc and r.random()<0.002: k=27
        if esc and r.random()<0.003: lines.append('5 000d'); lines.append('3 0064'); lines.append('2 000d')
        lines.append(f'{r.choice([0,0,1,2,3,5,8,13,30,60])} {k:04x}')
    open(f'{d}/s.txt','w').write('\n'.join(lines)+'\n')
    extra=['--nosb'] if nosb else []
    subprocess.run([H,'--cov','/tmp/claude-0/cov.bin' if not nosb else '/tmp/claude-0/covn.bin','--max-instr','3000000000','--script',f'{d}/s.txt','--log',f'{d}/h.txt','--dir',d,'--max-polls',str(polls)]+extra,capture_output=True)
    # port uses separate dir copy so files don't interfere
    d2=d+'p'; os.makedirs(d2,exist_ok=True); subprocess.run(['cp','/tmp/claude-0/d1/optiona_synth.dat',d2+'/optiona.dat'])
    try: os.remove(f'{d2}/savegame.bcw')
    except: pass
    pr=subprocess.run([P,'--test-script',f'{d}/s.txt','--log',f'{d}/p.txt','--dir',d2,'--max-polls',str(polls)]+extra,capture_output=True,text=True)
    h=open(f'{d}/h.txt').read().split('\n'); p=open(f'{d}/p.txt').read().split('\n')
    for i,(a,b) in enumerate(zip(h,p)):
        if a!=b: return (seed,nosb,'DIVERGE at line',i+1,a,b,pr.stderr[-300:])
    if len(h)!=len(p): return (seed,nosb,'LEN',len(h),len(p),pr.stderr[-300:])
    return (seed,nosb,'OK',len(h))
if __name__=='__main__':
    a,b,polls=int(sys.argv[1]),int(sys.argv[2]),int(sys.argv[3])
    nosb='nosb' in sys.argv; esc='esc' in sys.argv
    with cf.ThreadPoolExecutor(2) as ex:
        futs=[ex.submit(run,s,polls,nosb,esc) for s in range(a,b)]
        for f in cf.as_completed(futs):
            print(*f.result(), flush=True)
