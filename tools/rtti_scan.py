# Standalone RTTI / xref scanner used to produce ghidra/rtti.json. Run next to an extracted SLUS_203.26 (needs: pip install capstone).

import struct,sys
from capstone import *
D=open('SLUS_203.26','rb').read()
BASE=0x100000-0x1000   # vaddr = off + 0xFF000 for text/data/rodata
def off(v): return v-0xFF000
def rd(v,n): return D[off(v):off(v)+n]
def u32(v): return struct.unpack_from('<I',D,off(v))[0]
def cstr(v):
    o=off(v); e=D.index(b'\0',o); return D[o:e].decode('latin1')
def findstr(s):
    r=[];i=0;b=s.encode()
    while True:
        i=D.find(b,i)
        if i<0: break
        r.append(i+0xFF000); i+=1
    return r
md=Cs(CS_ARCH_MIPS,CS_MODE_MIPS64|CS_MODE_LITTLE_ENDIAN)
def dis(v,n=40):
    for i in md.disasm(rd(v,n*4),v):
        print('%08x  %-8s %s'%(i.address,i.mnemonic,i.op_str))
TEXT=(0x100000,0x30f960)
def words_eq(val,lo=0x31d300,hi=0x3bbb00):
    r=[]
    for v in range(lo,hi,4):
        if u32(v)==val: r.append(v)
    return r
def build_xrefs():
    """lui/addiu(ori) pair scan + gp-relative; returns dict addr->[code addrs]"""
    x={}
    lui={}
    for v in range(TEXT[0],TEXT[1],4):
        w=u32(v); op=w>>26; rs=(w>>21)&31; rt_=(w>>16)&31; imm=w&0xffff
        simm=imm-0x10000 if imm&0x8000 else imm
        if op==0x0f: lui[rt_]=(imm<<16,v); continue
        if op in(0x09,0x0d) and rs in lui and v-lui[rs][1]<=64:
            a=(lui[rs][0]+(simm if op==9 else imm))&0xffffffff
            x.setdefault(a,[]).append(v)
        elif op in (0x20,0x21,0x23,0x24,0x25,0x28,0x29,0x2b,0x31,0x39,0x37,0x3f,0x27) and rs in lui and v-lui[rs][1]<=64:
            a=(lui[rs][0]+simm)&0xffffffff
            x.setdefault(a,[]).append(v)
        # function boundary: jr ra resets
        if w==0x03e00008: lui={}
    return x
import pickle,os
def xrefs():
    if os.path.exists('x.pkl'): return pickle.load(open('x.pkl','rb'))
    x=build_xrefs(); pickle.dump(x,open('x.pkl','wb')); return x
import re,json
X=xrefs()
names={}
for m in re.finditer(rb'(?<![0-9A-Za-z_])(\d{1,2})([A-Za-z_][A-Za-z0-9_]*)\0',D[off(0x31d300):off(0x3bbb00)]):
    n=int(m.group(1)); s=m.group(2).decode()
    if len(s)==n: names[m.start()+0x31d300]=s
print(len(names),'candidate rtti names')
def fstart(v):
    for a in range(v,v-0x80,-4):
        w=u32(a)
        if (w>>16)==0x27bd and (w&0x8000): return a
    return None
tf={}   # tf func -> (name, tiobj, base tiobj)
for a,s in names.items():
    for x in X.get(a,[]):
        f=fstart(x)
        if not f: continue
        # parse lui/addiu inside function for regs
        reg={};ti=None;base=None;lui={}
        for v in range(f,f+0x80,4):
            w=u32(v);op=w>>26;rs=(w>>21)&31;rt_=(w>>16)&31;imm=w&0xffff;simm=imm-0x10000 if imm&0x8000 else imm
            if op==0xf: lui[rt_]=imm<<16
            elif op==9 and rs in lui:
                reg[rt_]=lui[rs]+simm
                if rt_!=rs: pass
            if w==0x03e00008: break
        # tiobj = reg s0 (16) typically; base = a2 (6)
        js=set()
        for v in range(f,f+0x60,4):
            w=u32(v)
            if w>>26==3: js.add((w&0x3ffffff)<<2)
            if w==0x03e00008: break
        if not js&{0x2f8e00,0x2f8e20,0x2f8dd8}: continue   # must call __rtti_si/_class/_user
        tf[f]=(s,reg.get(16),reg.get(6))
print(len(tf),'tf funcs')
ti2name={t[1]:t[0] for t in tf.values() if t[1]}
vt={}
for v in range(0x31d300,0x3bbb00-8,4):
    if u32(v+4) in tf and u32(v)==0:
        f=u32(v+4); ents=[]
        a=v+8
        while True:
            d,p=u32(a),u32(a+4)
            if not(TEXT[0]<=p<TEXT[1]) or (d&0xffff0000 and (d>>16)!=0) : break
            if p in tf: break
            ents.append((d,p)); a+=8
        vt.setdefault(tf[f][0],[]).append((v,ents))
print(len(vt),'classes with vtables')
out={'classes':{}}
for f,(s,ti,base) in tf.items():
    out['classes'][s]={'tf':f,'typeinfo':ti,'base':ti2name.get(base),'vtables':[{'addr':v,'funcs':[p for d,p in e]} for v,e in vt.get(s,[])]}
json.dump(out,open('rtti.json','w'),indent=1)
c=out['classes']['cDebugMenu']; print(c['base'],[hex(x) for x in c['vtables'][0]['funcs']] if c['vtables'] else None)
