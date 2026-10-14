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
    if names:
        print("%08x: %-8s $%s, $%s, $%s"%(a+i,names,R[rdd],R[rs],R[rt])); continue
    if op==0x1c and fn in (0x10,0x12) and rs==0 and rt==0:
        print("%08x: %-8s $%s"%(a+i,{0x10:"mfhi1",0x12:"mflo1"}[fn],R[rdd])); continue
    if op==0x1c and fn in (0x1a,0x1b) and rdd==0:
        print("%08x: %-8s $%s, $%s"%(a+i,{0x1a:"div1",0x1b:"divu1"}[fn],R[rs],R[rt])); continue
    if ins: print("%08x: %-8s %s"%(a+i,ins[0].mnemonic,ins[0].op_str))
    else: print("%08x: .word %08x"%(a+i,w))
