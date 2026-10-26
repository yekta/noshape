#!/bin/bash
# Every 30 min: delete stale test executables, and prune old artifact variants (keep the newest 6 per crate).
while true; do
  for d in /root/target-shared /root/target-behave /root/target-engine /root/target-qa; do
    find $d/debug/deps -maxdepth 1 -type f -executable ! -name '*.so' -mmin +45 -amin +30 -delete 2>/dev/null
  done
  find /root/target-lead/debug/deps -maxdepth 1 -type f -executable ! -name "*.so" -mmin +120 -amin +60 -delete 2>/dev/null
  python3 /tmp/lead/prune-build.py /root/target-shared /root/target-behave /root/target-engine /root/target-lead /root/target-qa >> /tmp/lead/disk-loop.log 2>&1
  python3 /tmp/lead/prune-deps.py /root/target-qa/debug/deps /root/target-shared/debug/deps /root/target-engine/debug/deps /root/target-behave/debug/deps /root/target-lead/debug/deps >> /tmp/lead/disk-loop.log 2>&1
  python3 /tmp/lead/fix-fingerprints.py /root/target-qa /root/target-shared /root/target-engine /root/target-behave /root/target-lead >> /tmp/lead/disk-loop.log 2>&1
  rm -rf /root/target-*/release 2>/dev/null
  # Linux perf's build-id cache: a copy of every profiled binary (18 GB on 10-26). Keep only the last 2 h.
  find /root/.debug -mindepth 1 -maxdepth 3 -mmin +120 -exec rm -rf {} + 2>/dev/null
  # Incremental caches (on since 10-25): drop per-crate caches untouched for 90 min.
  find /root/target-*/debug/incremental -mindepth 1 -maxdepth 1 -type d -mmin +90 -exec rm -rf {} + 2>/dev/null
  # Temp dirs left by e2e test processes that have exited (named by PID).
  for d in /tmp/noshape-ops-e2e-*; do p=${d##*-}; kill -0 $p 2>/dev/null || [ -z "$(find $d -maxdepth 0 -mmin +60)" ] || rm -rf "$d"; done
  # Old copies of the app binary that agents park in scratch dirs (12 h+).
  find /root/*-scratch /tmp -maxdepth 4 -type f -name "noshape*" -size +100M -mmin +120 -delete 2>/dev/null
  echo "$(date -u +%H:%M) $(df -h / | tail -1 | awk '{print $4}') free" >> /tmp/lead/disk-loop.log
  sleep 1800
done
