import struct,glob,os,json
def f24(b): return struct.unpack('<f',bytes([0x80,b[0],b[1],b[2]]))[0]
class Afl:
    def __init__(s,path):
        d=s.d=open(path,'rb').read()
        _,_,cnt,s.ptroff,s.dataoff=struct.unpack_from('<BBHII',d,0)
        s.P=struct.unpack_from('<%dI'%((s.dataoff-s.ptroff)//4),d,s.ptroff)
        s.anims=[];cur=None
        for i in range(cnt):
            o=12+i*36
            u0,pidx,ht,rel=struct.unpack_from('<IIBB',d,o); rest=struct.unpack_from('<11H',d,o+10)
            if ht in(255,205,49,45,2):
                cur=dict(hash=u0,frames=rest[0],events=rest[1:],groups=[],kind=ht); s.anims.append(cur)
            elif cur is not None: cur['groups'].append((ht,pidx))
    def curve(s,k,t):
        """value of pointer k at frame t; returns (value, bytes consumed)"""
        return s._eval(s.dataoff+s.P[k],t)[0]
    def _eval(s,o,t):
        d=s.d; h=d[o]|d[o+1]<<8; ty=h&15; n=h>>4
        if ty<=3:
            v=0.0
            for i in range(ty+1): v=v*t+f24(d[o+2+3*i:o+5+3*i])
            return v,2+3*(ty+1)
        if ty==5:
            p=o+2; tt=t; val=None
            for i in range(n):
                hh=d[p]|d[p+1]<<8; seg=hh>>4
                v,sz=s._eval(p,min(tt,seg) if i==n-1 else tt)
                if val is None and (tt<seg or i==n-1): val=v
                tt-=seg; p+=sz
            return val,p-o
        if ty==4:
            i=int(min(max(t,0),n-1)); return struct.unpack_from('<f',d,o+2+4*i)[0],2+4*n
        if ty==6:
            a,b=f24(d[o+2:o+5]),f24(d[o+5:o+8]); i=int(min(max(t,0),n-1)); return a+b*d[o+8+i],8+n
        if ty==7:
            a,b=f24(d[o+2:o+5]),f24(d[o+5:o+8]); i=int(min(max(t,0),n-1)); return a+b*struct.unpack_from('<H',d,o+8+2*i)[0],8+2*n
        raise ValueError('type %d at %x'%(ty,o))
def find(name): return glob.glob('/home/claude/lvl/ch/ANM/**/'+name,recursive=True)[0]
