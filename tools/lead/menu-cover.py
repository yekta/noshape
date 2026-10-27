#!/usr/bin/env python3
# Reimplements ops/tests/coverage.rs every_menu_item_is_listed on a git tree: prints missing / stale menu ids.
import subprocess, re, sys
R='/root/projects/noshape'
def git(*a): return subprocess.run(['git','-C',R,*a],capture_output=True,text=True).stdout
def labels(tree):
    out=set()
    files=[]
    for p in git('ls-tree','--name-only',tree,'crates/app/src/','crates/app-core/src/','crates/drawing-ui/src/','crates/assembly-ui/src/','crates/part-studio-ui/src/','crates/variable-studio-ui/src/','crates/history-ui/src/').split():
        n=p.split('/')[-1]
        area=('assembly' if n.startswith('assembly_') else 'drawing' if n.startswith('drawing_') else 'document' if (n.startswith('variable_') or n=='history_panel.rs') else 'partStudio' if n=='feature_dialog.rs' else 'view' if n=='camera_menu.rs' else None)
        if area and n.endswith('.rs'): files.append((area,p))
    for p in git('ls-tree','--name-only',tree,'crates/part-studio-ui/src/part_studio/','crates/app/src/part_studio/').split(): files.append(('partStudio',p))
    for area,p in files:
        t=git('show',f'{tree}:{p}')
        for pat in ['item("','OsMenuItem::new("']:
            i=0
            while True:
                k=t.find(pat,i)
                if k<0: break
                i=k+len(pat)
                if pat.startswith('item'):
                    b=t[k-1] if k>0 else ' '
                    if (b.isalnum() or b=='_') and not t[:k].endswith('icon_'): continue
                rest=t[i:]; e=0
                while e<len(rest) and not (rest[e]=='"' and (e==0 or rest[e-1]!='\\')): e+=1
                out.add(f'{area}:{rest[:e]}')
    return out
def listed(tree):
    t=git('show',f'{tree}:crates/ops/src/coverage.rs')
    return set(re.findall(r'\(\s*"((?:partStudio|assembly|drawing|document|view):[^"]*)"\s*,',t))
for name in sys.argv[1:]:
    tree=name
    if name!='origin/main':
        tree=git('merge-tree','--write-tree','origin/main',name).split()[0]
    L=labels(tree); M=listed(tree)
    print(f'{name}: labels={len(L)} listed={len(M)} missing={sorted(L-M)} stale={sorted(M-L)}')
