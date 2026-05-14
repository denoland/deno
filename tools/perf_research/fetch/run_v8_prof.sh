#!/usr/bin/env bash
# Run a Deno microbench with --v8-flags=--prof, then post-process via
# `node --prof-process` to produce a readable tick summary committed to
# profiles/.  This is the attribution mechanism used in the report.
#
# Usage:  bash run_v8_prof.sh
set -euo pipefail
cd "$(dirname "$0")"
DENO_BIN="${DENO_BIN:-../../../target/release/deno}"
NODE_BIN="${NODE_BIN:-node}"
mkdir -p profiles

run_prof() {
  local tag=$1 script=$2
  local workdir
  workdir=$(mktemp -d)
  echo "==> $tag: $DENO_BIN run -A --v8-flags=--prof $script (workdir=$workdir)"
  ( cd "$workdir" && "$DENO_BIN" run -A --no-prompt --v8-flags=--prof,--no-logfile-per-isolate "$OLDPWD/$script" ) > "profiles/${tag}.stdout.txt" 2> "profiles/${tag}.stderr.txt" || true
  local log
  log=$(find "$workdir" -maxdepth 1 -name '*-v8.log' -o -name 'v8.log' | head -1)
  if [ -z "$log" ]; then echo "no v8 log produced for $tag (workdir=$workdir)"; ls "$workdir"; return 1; fi
  cp "$log" "profiles/${tag}.v8.log"
  $NODE_BIN --prof-process "profiles/${tag}.v8.log" > "profiles/${tag}.prof.txt" 2>&1 || true
  echo "--- top entries for $tag ---"
  awk '/\[Bottom up \(heavy\) profile\]/{f=1} f' "profiles/${tag}.prof.txt" | head -40
}

run_prof headers_micro micro/headers_micro.js
run_prof request_response_micro micro/request_response_micro.js
