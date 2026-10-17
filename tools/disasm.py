"""Disassemble a range of the game ELF (TRICKY_ELF) with capstone: python3 -I tools/disasm.py 0x21ee70 232.
Needs `pip install capstone`. The R5900 extras (MMI, VU macro) decode as nonsense or MSA names; plain MIPS is right,
and `.word` forms like 00a22818 are the three-operand `mult rd,rs,rt`. Here the R5900 multiply family is decoded
from the word itself (lq/sq, mult/multu with rd, the pipeline-1 mult1/div1/mfhi1/mflo1, madd/maddu) so the destination shows.
"""
import sys,struct,os
from capstone import *
elf=open(os.environ["TRICKY_ELF"],'rb').read()
phoff=struct.unpack_from('<I',elf,0x1c)[0]; phn=struct.unpack_from('<H',elf,0x2c)[0]; phs=struct.unpack_from('<H',elf,0x2a)[0]
segs=[]
for i in range(phn):
    t,off,va,pa,fs,ms,fl,al=struct.unpack_from('<IIIIIIII',elf,phoff+i*phs)
    if t==1: segs.append((va,off,fs))
def rd(a,n):
    for va,off,fs in segs:
        if va<=a<va+fs: return elf[off+a-va:off+a-va+n]
    raise Exception('addr')
md=Cs(CS_ARCH_MIPS,CS_MODE_MIPS64+CS_MODE_LITTLE_ENDIAN)
md.detail=False
a=int(sys.argv[1],16); n=int(sys.argv[2],0)
code=rd(a,n)
# delay-slot labelled style
for i in range(0,n,4):
    w=struct.unpack_from('<I',code,i)[0]
    ins=list(md.disasm(code[i:i+4],a+i))
    R=["zero","at","v0","v1","a0","a1","a2","a3","t0","t1","t2","t3","t4","t5","t6","t7","s0","s1","s2","s3","s4","s5","s6","s7","t8","t9","k0","k1","gp","sp","fp","ra"]
    op=w>>26; rs=(w>>21)&31; rt=(w>>16)&31; rdd=(w>>11)&31; fn=w&63
    names=None
    if op==0 and fn in (0x18,0x19) and (w>>6)&31==0: names={0x18:"mult",0x19:"multu"}[fn]
    elif op==0x1c and (w>>6)&31==0 and fn in (0,1,0x18,0x19): names={0:"madd",1:"maddu",0x18:"mult1",0x19:"multu1"}[fn]
    if op in (0x1e, 0x1f):
        off=w&0xffff
        off=off-0x10000 if off&0x8000 else off
        print("%08x: %-8s $%s, %s($%s)"%(a+i,"lq" if op==0x1e else "sq",R[rt],hex(off) if off>=0 else "-"+hex(-off),R[rs])); continue
    # VU0 macro mode (as tools/r5900's exec.rs runs it): lqc2/sqc2, qmfc2/cfc2/qmtc2, special1 and special2
    if op in (0x36, 0x3e):
        off=w&0xffff
        off=off-0x10000 if off&0x8000 else off
        print("%08x: %-8s vf%d, %s($%s)"%(a+i,"lqc2" if op==0x36 else "sqc2",rt,hex(off) if off>=0 else "-"+hex(-off),R[rs])); continue
    if op==0x12:
        if rs&16==0 and rs in (1,2,5):
            nm={1:"qmfc2",2:"cfc2",5:"qmtc2"}[rs]
            print("%08x: %-8s $%s, %s%d"%(a+i,nm,R[rt],"vi" if rs==2 else "vf",rdd)); continue
        if rs&16:
            dest=''.join(c for k,c in enumerate("xyzw") if (w>>21)&15&(8>>k))
            ft,fs,fd,sa=rt,rdd,(w>>6)&31,(w>>6)&31
            bc="xyzw"[fn&3]
            if fn<60:
                m={40:"vadd",42:"vmul",44:"vsub",46:"vopmsub",28:"vmulq"}.get(fn)
                if m is None and fn<4: m="vadd"+bc
                if m is None and 8<=fn<12: m="vmadd"+bc
                if m is None and 24<=fn<28: m="vmul"+bc
                if m is None and 16<=fn<20: m="vmax"+bc
                if m is None and 20<=fn<24: m="vmini"+bc
                if m:
                    tail="" if fn==28 else ", vf%d"%ft
                    print("%08x: %-8s vf%d, vf%d%s"%(a+i,m+"."+dest,fd,fs,tail)); continue
            else:
                k=(sa<<2)|(fn&3)
                fsf="xyzw"[(w>>21)&3]; ftf="xyzw"[(w>>23)&3]
                m={42:"vmula",46:"vopmula"}.get(k)
                if m is None and k<4: m="vadda"+"xyzw"[k]
                if m is None and 8<=k<12: m="vmadda"+"xyzw"[k-8]
                if m is None and 24<=k<28: m="vmula"+"xyzw"[k-24]
                if m:
                    print("%08x: %-8s ACC, vf%d, vf%d"%(a+i,m+"."+dest,fs,ft)); continue
                if k==48: print("%08x: %-8s vf%d, vf%d"%(a+i,"vmove."+dest,ft,fs)); continue
                if k in (56,58): print("%08x: %-8s Q, vf%d.%s, vf%d.%s"%(a+i,{56:"vdiv",58:"vrsqrt"}[k],fs,fsf,ft,ftf)); continue
                if k==57: print("%08x: %-8s Q, vf%d.%s"%(a+i,"vsqrt",ft,ftf)); continue
                if k==59: print("%08x: vwaitq"%(a+i)); continue
                if 20<=k<24: print("%08x: %-8s vf%d, vf%d"%(a+i,"vftoi%d.%s"%([0,4,12,15][k-20],dest),ft,fs)); continue
                if k==60: print("%08x: %-8s vi%d, vf%d.%s"%(a+i,"vmtir",ft,fs,fsf)); continue
    if names:
        print("%08x: %-8s $%s, $%s, $%s"%(a+i,names,R[rdd],R[rs],R[rt])); continue
    if op==0x1c and fn in (0x10,0x12) and rs==0 and rt==0:
        print("%08x: %-8s $%s"%(a+i,{0x10:"mfhi1",0x12:"mflo1"}[fn],R[rdd])); continue
    if op==0x1c and fn in (0x1a,0x1b) and rdd==0:
        print("%08x: %-8s $%s, $%s"%(a+i,{0x1a:"div1",0x1b:"divu1"}[fn],R[rs],R[rt])); continue
    if ins: print("%08x: %-8s %s"%(a+i,ins[0].mnemonic,ins[0].op_str))
    else: print("%08x: .word %08x"%(a+i,w))
