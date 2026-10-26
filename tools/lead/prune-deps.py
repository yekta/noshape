import os, re, sys, time
# Delete build artifacts nobody has written or read for 48h+. With relatime, atime can be up to 24h stale,
# so 48h means "not used in at least a day". Never touches anything a running build could need.
now=time.time(); freed=0; n=0; AGE=48*3600
for d in sys.argv[1:]:
    for f in os.scandir(d):
        if not f.is_file() or not re.search(r'-[0-9a-f]{16}(\.|$)', f.name): continue
        st=f.stat()
        if now-st.st_mtime>AGE and now-st.st_atime>AGE:
            try: os.remove(f.path); freed+=st.st_size; n+=1
            except OSError: pass
print(f"removed {n} files, {freed/1e9:.1f} GB")
