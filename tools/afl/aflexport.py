"""Decode an SSX Tricky .afl animation file to JSON.
usage: aflexport.py <file.afl> <names.json key or -> <out.json>
Each clip: name, frames, data = frames x 66 floats:
  [0..3)   hips position (cm)            [3..60) 19 bone rotations (Euler x,y,z; hips .. r_foot)
  [60..63) board position (cm)           [63..66) board rotation (Euler)"""
import sys,json,os; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from afl import Afl
a=Afl(sys.argv[1]); names=json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'animnames.json'))).get(os.path.basename(sys.argv[1]),[])
clips=[]
for i,an in enumerate(a.anims):
    g0=[p for t,p in an['groups'] if t==0]; g1=[p for t,p in an['groups'] if t==1]
    if not g0 or not g1 or an['frames']==0: continue
    data=[]
    for f in range(an['frames']):
        data+= [round(a.curve(g0[0]+k,f),4) for k in range(60)]+[round(a.curve(g1[0]+k,f),4) for k in range(6)]
    clips.append(dict(name=(names[i] if i<len(names) and names[i] else 'anim_%d'%i),frames=an['frames'],data=data))
json.dump(dict(clips=clips),open(sys.argv[2],'w'),separators=(',',':'))
print(sys.argv[2],len(clips),'clips',os.path.getsize(sys.argv[2])//1024,'KB')
