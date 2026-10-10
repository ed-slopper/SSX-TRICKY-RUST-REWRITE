"""Decode the trick book (DATA/TUTORIAL/TRICKDEF.DAT) into chars/trickbook.json for tricky-rs.
usage: trickbook.py SLUS_203.26 TRICKDEF.DAT [out_dir]   (names come from the game executable's string tables)"""
import struct
import sys
D=open(sys.argv[1],'rb').read()
def off(v): return v-0xFF000
def u32(v): return struct.unpack_from('<I',D,off(v))[0]
def cstr(v):
    o=off(v); e=D.index(b'\0',o); return D[o:e].decode('latin1')
def ptrtab(v,n): return [cstr(u32(v+4*i)) if u32(v+4*i) else None for i in range(n)]

import struct, json
d=open(sys.argv[2],'rb').read()
T=lambda a,n:[(s or '') for s in ptrtab(a,n)]
RAILT=T(0x320b08,5); RAILSPIN=T(0x320b20,11)+['']*5; STANCE=T(0x320970,5)+['']*11
SPDIR=T(0x320988,3)+['']; SPIN=T(0x320998,11)+['']*5; FLIPN=T(0x3209c8,8)
FLIPT=T(0x3209d8,5)+['']*11; COMBO=T(0x3209f0,45)
SPIN2=T(0x320aa8,11)+['']*5; GR=T(0x320b58,102)+['']*154; TOL=['','To Late ','','']
SUF=T(0x320ad8,4)+['']*12; CRASH=T(0x320ae8,8)+['']*8
CHARS=['Eddie','Kaori','Luther','Mac','Moby','Zoe','JP','Elise','Psymon','Seeiah','Brodi','Marisol']
def name(w0,w1):
    parts=[RAILT[(w1>>12)&0xf] if ((w1>>12)&0xf)<5 else '', RAILSPIN[(w1>>4)&0xf], STANCE[(w1>>16)&0xf],
      SPDIR[(w1>>27)&7] if ((w1>>27)&7)<4 else '', SPIN[(w0>>24)&0xf], FLIPN[(w1>>24)&7], FLIPT[(w1>>8)&0xf],
      COMBO[w0&0xff] if (w0&0xff)<45 else '?', SPIN2[w0>>28], GR[(w0>>8)&0xff], TOL[w1>>30], GR[(w0>>16)&0xff],
      SUF[(w1>>20)&0xf], CRASH[w1&0xf]]
    return ''.join(parts).strip()
out={}; md=['# SSX Tricky trick book (trickdef.dat) decoded','',
 'Names rendered with Score_FormatTrickName tables (0x1551c0). spin + = BS (as stored; display flips for goofy riders), flip + = front.','']
for s in range(12):
    lst=[]
    md+=[f'## Set {s} — {CHARS[s]}','']
    for t in range(30):
        w0,w1,flip,spin,rail,pad,p1,p2,sl,sl2=struct.unpack_from('<IIhhhhhhII',d,s*0x348+t*28)
        g1=(w0>>8)&0xff; g2=(w0>>16)&0xff
        stance=(w1>>16)&0xf
        n=name(w0,w1)
        e=dict(chapter=t//5+1,index=t,name=n,spin_deg=spin*180,flip_deg=flip*360,
               grab=GR[g1].strip() or None,grab2=GR[g2].strip() or None,
               rail=bool(rail) or bool((w1>>12)&0xf) or stance in (2,3),
               rail_spin_deg=((w1>>4)&0xf)*180 if rail else 0,
               switch=stance in (1,3),uber=g1>=53 or g2>=53,late=stance==4,
               tweak=(27<=g1<=52) or (27<=g2<=52),landing=SUF[(w1>>20)&0xf] or None,
               combo_name=COMBO[w0&0xff].strip() or None,
               slot=sl,slot2=None if sl2==26 else sl2,plus=p1,plus2=p2,
               raw=dict(trickId=f'{w0:08x}',flags=f'{w1:08x}',rec=d[s*0x348+t*28:s*0x348+t*28+28].hex()))
        lst.append(e)
        if t%5==0: md+=[f'**Chapter {t//5+1}**','']
        md.append(f'- {t:2d}. {n}  `({w0:08x} {w1:08x}; spin {spin*180:+d}, flip {flip*360:+d})`')
        if t%5==4: md.append('')
    out[f'set{s}_{CHARS[s]}']=lst
import os; od=sys.argv[3] if len(sys.argv)>3 else '.'
json.dump(out,open(os.path.join(od,'trickbook.json'),'w'),indent=1)
open(os.path.join(od,'trickbook.md'),'w').write('\n'.join(md)+'\n')
