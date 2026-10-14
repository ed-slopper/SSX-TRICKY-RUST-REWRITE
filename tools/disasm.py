"""Disassemble a range of the game ELF (TRICKY_ELF) with capstone: python3 -I tools/disasm.py 0x21ee70 232.
Needs `pip install capstone`. The R5900 extras (MMI, VU macro) decode as nonsense or MSA names; plain MIPS is right,
and `.word` forms like 00a22818 are the three-operand `mult rd,rs,rt`.
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
    if ins: print("%08x: %-8s %s"%(a+i,ins[0].mnemonic,ins[0].op_str))
    else: print("%08x: .word %08x"%(a+i,w))
