"""Pull .afl files out of DATA/CHAR/ANM.BIG (EA C0FB archive, RefPack-compressed entries).
usage: anmbig.py ANM.BIG out_dir name.afl [name.afl ...]   (names case-insensitive; no names = all)"""
import sys,os,struct; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
import refpack
d=open(sys.argv[1],'rb').read(); out=sys.argv[2]; want={n.lower() for n in sys.argv[3:]}
os.makedirs(out,exist_ok=True)
n=struct.unpack('>H',d[4:6])[0]; p=6
for i in range(n):
    eo=int.from_bytes(d[p:p+3],'big'); es=int.from_bytes(d[p+3:p+6],'big'); p+=6
    e=d.index(b'\0',p); name=d[p:e].decode('latin1'); p=e+1
    base=name.replace('\\','/').split('/')[-1]
    if want and base.lower() not in want: continue
    raw=d[eo:eo+es]
    data=refpack.unpack(raw)[0] if raw[1]==0xfb else raw
    open(os.path.join(out,base),'wb').write(data); print(base,len(data))
