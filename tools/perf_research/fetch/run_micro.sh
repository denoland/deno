#!/usr/bin/env bash
# Run microbenches in all three runtimes.
# Usage: bash run_micro.sh
set -euo pipefail
cd "$(dirname "$0")"

DENO_BIN="${DENO_BIN:-../../../target/release/deno}"
NODE_BIN="${NODE_BIN:-node}"
BUN_BIN="${BUN_BIN:-bun}"

OUT="micro_results.jsonl"
: > "$OUT"

run_one() {
  local script=$1
  echo "===== $script ====="
  for rt in deno node bun; do
    case $rt in
      deno) cmd="$DENO_BIN run -A --no-prompt $script" ;;
      node) cmd="$NODE_BIN $script" ;;
      bun)  cmd="$BUN_BIN $script" ;;
    esac
    while IFS= read -r line; do
      printf '%s\n' "$line" | jq --arg rt "$rt" --arg script "$(basename $script)" '. + {runtime: $rt, script: $script}' >> "$OUT" 2>/dev/null || \
        printf '{"runtime":"%s","script":"%s","raw":%s}\n' "$rt" "$(basename $script)" "$(printf '%s' "$line" | jq -Rs .)" >> "$OUT"
    done < <($cmd 2>/dev/null)
  done
}

run_one "micro/headers_micro.js"
run_one "micro/request_response_micro.js"

echo
echo "--- summary ---"
cat "$OUT"
