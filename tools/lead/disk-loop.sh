#!/bin/bash
# Every 30 min: delete stale test executables, and prune old artifact variants (keep the newest 6 per crate).
while true; do
  for d in /root/target-core /root/target-psui /root/target-skasm /root/target-dx; do
    find $d/debug/deps -maxdepth 1 -type f -executable ! -name '*.so' -mmin +45 -amin +30 -delete 2>/dev/null
  done
  find /root/target-lead/debug/deps -maxdepth 1 -type f -executable ! -name "*.so" -mmin +120 -amin +60 -delete 2>/dev/null
  python3 /tmp/lead/prune-build.py /root/target-core /root/target-psui /root/target-skasm /root/target-dx /root/target-lead >> /tmp/lead/disk-loop.log 2>&1
  python3 /tmp/lead/prune-deps.py /root/target-core/debug/deps /root/target-psui/debug/deps /root/target-skasm/debug/deps /root/target-dx/debug/deps /root/target-lead/debug/deps >> /tmp/lead/disk-loop.log 2>&1
  python3 /tmp/lead/fix-fingerprints.py /root/target-core /root/target-psui /root/target-skasm /root/target-dx /root/target-lead >> /tmp/lead/disk-loop.log 2>&1
  # Release builds (perf checks) untouched for 3 h.
  for d in /root/target-*/release; do [ -d "$d" ] && [ -z "$(find $d -mmin -180 -print -quit)" ] && rm -rf "$d"; done
  # Linux perf's build-id cache: a copy of every profiled binary (18 GB on 10-26). Keep only the last 2 h.
  find /root/.debug -mindepth 1 -maxdepth 3 -mmin +120 -exec rm -rf {} + 2>/dev/null
  # Incremental caches (on again since 10-26 20:00): drop per-crate caches untouched for 90 min, and keep each target
  # dir's under 20 GB (oldest first).
  find /root/target-*/debug/incremental -mindepth 1 -maxdepth 1 -type d -mmin +90 -exec rm -rf {} + 2>/dev/null
  for d in /root/target-*/debug/incremental; do
    while [ "$(du -s $d 2>/dev/null | cut -f1)" -gt 20000000 ]; do o=$(ls -tr $d | head -1); [ -z "$o" ] && break; rm -rf "$d/$o"; done
  done
  # Temp dirs left by e2e test processes that have exited (named by PID).
  for d in /tmp/noshape-ops-e2e-*; do p=${d##*-}; kill -0 $p 2>/dev/null || [ -z "$(find $d -maxdepth 0 -mmin +60)" ] || rm -rf "$d"; done
  # Old copies of the app binary that agents park in scratch dirs (12 h+).
  find /root/*-scratch /tmp -maxdepth 4 -type f -name "noshape*" -size +100M -mmin +120 -delete 2>/dev/null
  echo "$(date -u +%H:%M) $(df -h / | tail -1 | awk '{print $4}') free" >> /tmp/lead/disk-loop.log
  sleep 1800
done
