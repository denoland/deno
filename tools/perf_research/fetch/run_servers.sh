#!/usr/bin/env bash
# Launch all three servers and benchmark them with wrk on each route.
# Outputs JSON-lines results to results.jsonl in this directory.
#
# Usage: bash run_servers.sh [duration_seconds] [connections]
#
# Requires: ./target/release/deno, node, bun, wrk on PATH.

set -euo pipefail
cd "$(dirname "$0")"

DENO_BIN="${DENO_BIN:-../../../target/release/deno}"
NODE_BIN="${NODE_BIN:-node}"
BUN_BIN="${BUN_BIN:-bun}"

DURATION="${1:-10}"
CONNS="${2:-64}"
THREADS=4

OUT="results.jsonl"
: > "$OUT"

versions() {
  echo "deno=$($DENO_BIN --version | head -1)"
  echo "node=$($NODE_BIN --version)"
  echo "bun=$($BUN_BIN --version)"
}
versions > versions.txt
cat versions.txt

start_server() {
  local cmd=$1
  local logfile=$2
  bash -c "$cmd" > "$logfile" 2>&1 &
  echo $!
}

wait_port() {
  local port=$1
  for i in $(seq 1 50); do
    if exec 3<>/dev/tcp/127.0.0.1/$port 2>/dev/null; then
      exec 3<&-; exec 3>&-
      return 0
    fi
    sleep 0.2
  done
  echo "port $port never came up"
  return 1
}

run_wrk() {
  local label=$1 url=$2 method=${3:-GET} body=${4:-}
  local extra=""
  if [ "$method" = "POST" ]; then
    extra="-s post.lua"
    cat > post.lua <<EOF
wrk.method = "POST"
wrk.body   = "$body"
wrk.headers["Content-Type"] = "application/json"
EOF
  fi
  echo "==> $label  $method $url"
  out=$(wrk -t$THREADS -c$CONNS -d${DURATION}s --latency $extra "$url" 2>&1 || true)
  echo "$out"
  rps=$(echo "$out" | awk '/Requests\/sec/ {print $2}')
  lat_avg=$(echo "$out" | awk '/^    Latency/ {print $2}')
  lat_p99=$(echo "$out" | awk '/99%/ {print $2}')
  printf '{"label":"%s","method":"%s","url":"%s","rps":"%s","lat_avg":"%s","lat_p99":"%s"}\n' \
    "$label" "$method" "$url" "$rps" "$lat_avg" "$lat_p99" >> "$OUT"
}

run_runtime() {
  local name=$1 port=$2 cmd=$3
  local logfile="server_${name}.log"
  echo "===== $name on port $port ====="
  pid=$(start_server "$cmd" "$logfile")
  trap "kill $pid 2>/dev/null || true" EXIT
  wait_port "$port" || { kill $pid; return; }
  sleep 1

  run_wrk "${name}_hello"      "http://127.0.0.1:${port}/hello"
  run_wrk "${name}_headers"    "http://127.0.0.1:${port}/headers"
  run_wrk "${name}_echo_small" "http://127.0.0.1:${port}/echo" POST '{"a":1,"b":2,"c":[1,2,3]}'

  kill $pid 2>/dev/null || true
  wait 2>/dev/null || true
  trap - EXIT
}

run_runtime deno 8080 "$DENO_BIN run -A --no-prompt servers/deno_server.js --port=8080"
run_runtime node 8081 "$NODE_BIN servers/node_server.mjs --port=8081"
run_runtime bun  8082 "$BUN_BIN  servers/bun_server.js --port=8082"

echo
echo "--- summary ---"
cat "$OUT"
