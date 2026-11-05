#!/bin/bash
# tools/bool-share.sh [corpus...]: how far noshape's own Boolean goes: of the recorded Booleans, the share it takes
# (by count) and the share of OCCT's time those are (OCCT run there, each case once). One "share" row per corpus, then
# one for all; a case in several corpora counts in the first. Run from a worktree; the corpora are directories under
# ~/agents/bool-corpora on the build host (NOSHAPE_RB_HOST, default vibe4). Default: the parity documents' (parity56,
# first, so its row is theirs alone), bool-corpus, corpus5, the shell recordings and the knurl sweep.
# docs/parity/SCORE.md's Boolean table takes the parity56 and all rows.
set -e
cd "$(git rev-parse --show-toplevel)"
want=${*:-parity56 bool-corpus corpus5 shfall sh-new2 shv shell6 kall 'k[0-9]*' 'kg-*' 'kh*' 'kt-*'}
NOSHAPE_RB_HOST=${NOSHAPE_RB_HOST:-vibe4} exec tools/tiers/rb.sh nice -n 10 bash -c '
  dirs=$(cd ~/agents/bool-corpora && for w in '"$want"'; do [ -d "$w" ] && echo "$HOME/agents/bool-corpora/$w"; done | awk "!seen[\$0]++" | paste -sd:)
  NOSHAPE_BOOL_CORPORA=$dirs NOSHAPE_BOOLEAN_MEMORY_MB=16000 cargo test --release -p noshape-kernel --test all -- brep_replay::brep_corpus_share --ignored --nocapture --test-threads=1 2>&1 | grep -oE "share .*|panicked.*|error.*|signal.*"'
