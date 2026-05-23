#!/usr/bin/env bash
# Copyright 2018-2026 the Deno authors. MIT license.
#
# Runs the ext/websocket fast-TCP path benchmark in the same load
# shapes as fastwebsockets PR #133's bench: conns/payload pairs of
# 100/20, 10/1024, 10/16384, 200/16384, 500/16384. Each shape runs
# twice (1× warmup, 1× measure) on both the fast path and the
# generic fallback path (via DENO_WS_DISABLE_FAST_TCP=1) and prints
# msg/s plus relative speedup.
#
# Usage:
#   ./tools/ws_bench/run_bench.sh /path/to/release/deno
#
# Requires the standalone load generator built once with
#   (cd tools/ws_bench && cargo build --release)
set -euo pipefail

DENO_BIN="${1:?usage: $0 /path/to/deno}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LT="$ROOT/tools/ws_bench/target/release/ws_load_test"
SERVER="$ROOT/tools/ws_bench/echo_server.ts"

if [[ ! -x "$DENO_BIN" ]]; then
  echo "deno binary not found: $DENO_BIN" >&2; exit 1
fi
if [[ ! -x "$LT" ]]; then
  echo "load generator not built; run (cd tools/ws_bench && cargo build --release)" >&2
  exit 1
fi

SHAPES=(
  "100 20"
  "10 1024"
  "10 16384"
  "200 16384"
  "500 16384"
)

run_one() {
  local label="$1"; shift
  local conns="$1"; shift
  local payload="$1"; shift
  local extra_env="$1"; shift
  local addr="127.0.0.1:$((10000 + RANDOM % 50000))"
  # shellcheck disable=SC2086 # we want word-split on extra_env
  env $extra_env FWS_ADDR="$addr" "$DENO_BIN" run --allow-net --allow-env "$SERVER" >/dev/null 2>&1 &
  local pid=$!
  sleep 1
  local result
  result=$("$LT" "$conns" "$payload" 5 "$addr" 2>/dev/null | tail -1) || true
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  local mps
  mps=$(echo "$result" | sed -n 's/.*mps=\([0-9]*\).*/\1/p')
  printf "%-22s conns=%-3s payload=%-6s mps=%s\n" "$label" "$conns" "$payload" "${mps:-?}"
  echo "${mps:-0}"
}

echo "shape         | fast (mps)  | base (mps)  | speedup"
echo "--------------+-------------+-------------+--------"
for shape in "${SHAPES[@]}"; do
  IFS=' ' read -r conns payload <<<"$shape"
  fast_mps=$(run_one "fast"     "$conns" "$payload" "" | tail -1)
  base_mps=$(run_one "baseline" "$conns" "$payload" "DENO_WS_DISABLE_FAST_TCP=1" | tail -1)
  if [[ "${base_mps:-0}" -gt 0 ]]; then
    speed=$(awk -v f="$fast_mps" -v b="$base_mps" 'BEGIN{printf "%.3fx", f/b}')
  else
    speed="?"
  fi
  printf "%4d/%-6d   | %11s | %11s | %s\n" "$conns" "$payload" "$fast_mps" "$base_mps" "$speed"
done
