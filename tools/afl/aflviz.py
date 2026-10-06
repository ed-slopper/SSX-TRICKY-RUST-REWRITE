import sys; sys.path.insert(0,'/home/claude/lvl')
from afl import *
import numpy as np, matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt
bones=json.load(open('/home/claude/lvl/chars/mac/model.json'))['bones']
def Rx(a): c,s=np.cos(a),np.sin(a); return np.array([[1,0,0],[0,c,-s],[0,s,c]])
def Ry(a): c,s=np.cos(a),np.sin(a); return np.array([[c,0,s],[0,1,0],[-s,0,c]])
def Rz(a): c,s=np.cos(a),np.sin(a); return np.array([[c,-s,0],[s,c,0],[0,0,1]])
def R(r): return Rz(-r[2])@Ry(-r[1])@Rx(-r[0])
def pose(a,an,f):
    g0=[p for t,p in an['groups'] if t==0][0]
    v=np.array([a.curve(g0+k,f) for k in range(60)]).reshape(20,3)
    W=[]
    for i,b in enumerate(bones[:19]):
        M=np.eye(4); M[:3,:3]=R(v[i+1]); M[:3,3]=b['pos'] if i>0 else v[0]
        W.append(M if b['parent']<0 else W[b['parent']]@M)
    return np.array([w[:3,3] for w in W]),W
def sheet(a,ids,out,cols=10):
    rows=(len(ids)+cols-1)//cols
    fig,axs=plt.subplots(rows,cols,figsize=(cols*2.1,rows*2.4),dpi=70); axs=np.array(axs).reshape(-1)
    for ax in axs: ax.axis('off')
    for ax,i in zip(axs,ids):
        an=a.anims[i]; n=an['frames']
        for f,c in ((0,'#bbbbbb'),(n//2,'#5599ff'),(max(n-1,0),'k')):
            try: P,_=pose(a,an,f)
            except Exception as e: ax.set_title('%d ERR'%i,fontsize=7); break
            P=P-P[0]*[1,1,0]   # keep height, drop horizontal travel
            for j,b in enumerate(bones[:19]):
                if b['parent']>=0: q=P[b['parent']]; ax.plot([q[0],P[j][0]],[q[2],P[j][2]],c=c,lw=1.3)
            ax.plot(P[4][0],P[4][2]+8,'o',c=c,ms=5)
        ax.set_xlim(-90,90); ax.set_ylim(-20,200); ax.set_aspect('equal'); ax.set_title('%d  %df'%(i,n),fontsize=8)
    plt.tight_layout(); plt.savefig(out); plt.close()
if __name__=='__main__':
    a=Afl(find(sys.argv[1])); lo,hi=int(sys.argv[2]),int(sys.argv[3]); sheet(a,list(range(lo,min(hi,len(a.anims)))),sys.argv[4])
