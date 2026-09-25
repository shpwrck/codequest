#!/usr/bin/env bash
# Measure live question-generation quality: run the app's exact generation path
# (project brief, bounded prompt, provider CLI on stdin, acceptance policy)
# against a repository and print every candidate as ACCEPT or REJECT with the
# limits it violated, plus answer-position, lens, focus, and PREDICT tallies
# and the acceptance rate. Each level costs one real provider request.
#
# Usage: scripts/eval-questions.sh [claude|codex] [levels] [repository]
#   scripts/eval-questions.sh claude 1,4
#   CQA_CODEX_MODEL=gpt-5.5 scripts/eval-questions.sh codex 1,2,4 ~/src/other-repo
#
# Arguments override CQ_EVAL_PROVIDER, CQ_EVAL_LEVELS, and CQ_EVAL_REPO.
# CQ_EVAL_ROUNDS, CQ_EVAL_COUNT, CQ_EVAL_TIMEOUT_SECS, and CQ_EVAL_LEARNER=save
# are read as documented in src-tauri/src/question_eval.rs, and the provider
# binary and model come from CQA_CLAUDE, CQA_CODEX, CQA_CLAUDE_MODEL, and
# CQA_CODEX_MODEL, exactly as in the app.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export CQ_EVAL_PROVIDER="${1:-${CQ_EVAL_PROVIDER:-claude}}"
export CQ_EVAL_LEVELS="${2:-${CQ_EVAL_LEVELS:-1,4}}"
if [ -n "${3:-}" ]; then
  CQ_EVAL_REPO="$(cd "$3" && pwd)"
  export CQ_EVAL_REPO
fi

exec cargo test --manifest-path "$root/src-tauri/Cargo.toml" --lib \
  question_eval::live_question_generation_quality -- --ignored --nocapture
