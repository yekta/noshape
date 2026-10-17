import re, os, sys
import xml.etree.ElementTree as ET
D='/root/noshape/crates/app/assets/icons'
def pts_from_d(d):
    toks=re.findall(r'[MLCQHVAZmlcqhvaz]|-?\d*\.?\d+(?:e-?\d+)?',d)
    out=[];cmd=None;i=0;cur=(0,0);nums=[]
    def flush(cmd,nums):
        nonlocal cur
        if cmd in 'ML' or cmd in 'CQ':
            for j in range(0,len(nums)-1,2):
                cur=(nums[j],nums[j+1]);out.append(cur)
        elif cmd=='H':
            for n in nums: cur=(n,cur[1]);out.append(cur)
        elif cmd=='V':
            for n in nums: cur=(cur[0],n);out.append(cur)
        elif cmd=='A':
            for j in range(0,len(nums),7):
                if j+6<len(nums): cur=(nums[j+5],nums[j+6]);out.append(cur)
        elif cmd=='a':
            for j in range(0,len(nums),7):
                if j+6<len(nums): cur=(cur[0]+nums[j+5],cur[1]+nums[j+6]);out.append(cur)
    for t in toks:
        if t.isalpha():
            if cmd: flush(cmd,nums)
            cmd=t;nums=[]
        else: nums.append(float(t))
    if cmd: flush(cmd,nums)
    return out
bad=[]
for fn in sorted(os.listdir(D)):
    if not fn.endswith('.svg'): continue
    tree=ET.parse(os.path.join(D,fn))
    ps=[]
    for el in tree.iter():
        tag=el.tag.split('}')[-1]
        # skip elements inside transformed groups
        if tag=='path': ps+=pts_from_d(el.get('d'))
        elif tag=='circle':
            cx,cy,r=float(el.get('cx')),float(el.get('cy')),float(el.get('r'));ps+=[(cx-r,cy-r),(cx+r,cy+r)]
        elif tag=='ellipse':
            cx,cy,rx,ry=[float(el.get(k)) for k in ('cx','cy','rx','ry')];ps+=[(cx-rx,cy-ry),(cx+rx,cy+ry)]
        elif tag=='rect':
            x,y,w,h=[float(el.get(k)) for k in ('x','y','width','height')];ps+=[(x,y),(x+w,y+h)]
    xs=[p[0] for p in ps];ys=[p[1] for p in ps]
    has_t='transform' in open(os.path.join(D,fn)).read()
    if not ps: continue
    mn=min(min(xs),min(ys));mx=max(max(xs),max(ys))
    if (mn<1.0 or mx>23.0) and not has_t:
        bad.append((fn,round(min(xs),2),round(min(ys),2),round(max(xs),2),round(max(ys),2)))
for b in bad: print(*b)
