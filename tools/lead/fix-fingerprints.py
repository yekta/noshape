import os, sys, shutil
# Delete fingerprint dirs whose lib outputs are gone from deps, so cargo rebuilds them instead of failing.
n=0
for t in sys.argv[1:]:
    fp=os.path.join(t,'debug/.fingerprint'); deps=os.path.join(t,'debug/deps')
    have=set(os.listdir(deps))
    for d in os.listdir(fp):
        p=os.path.join(fp,d)
        libs=[f for f in os.listdir(p) if f.startswith('lib-') and not f.endswith('.json')]
        if not libs: continue
        name,h=d.rsplit('-',1); crate=libs[0][4:]
        if not any(x.startswith(f'lib{crate}-{h}.') for x in have):
            shutil.rmtree(p,ignore_errors=True); n+=1
print('removed',n,'stale fingerprints')
