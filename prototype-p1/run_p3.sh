#!/bin/bash
# PROTOTYPE — wayfinder P3. Throwaway.
# Builds the three release binaries P3 compares and runs the cost matrix.
#
#   ./run_p3.sh build   # ~15 min per variant, three variants
#   ./run_p3.sh bench
#   ./run_p3.sh startup
set -euo pipefail
cd "$(dirname "$0")"
ROOT=$(cd .. && pwd)
BIN=./bin
mkdir -p "$BIN"

build() {
  local name=$1
  shift
  echo "=== building $name: cargo build --release --bin deno $* ==="
  (cd "$ROOT" && cargo build --release --bin deno "$@")
  cp "$ROOT/target/release/deno" "$BIN/deno-$name"
  echo "=== $name -> $BIN/deno-$name ==="
}

case "${1:-bench}" in
build)
  # `off` doubles as the `accessor` binary: accessors install on the live
  # global after snapshot deserialization, so they need no build-time feature.
  build off
  build mask --features permcap_mask
  build nonmask --features permcap_nonmask
  ;;

bench)
  cd app
  echo "################ off (no handler installed at all) ################"
  ../bin/deno-off run -A e6_install_cost.mjs

  echo "################ accessor (PERMCAP_ACCESSOR=1, same binary) #######"
  PERMCAP=1 PERMCAP_ACCESSOR=1 ../bin/deno-off run -A e6_install_cost.mjs

  echo "################ mask, callback disabled (PERMCAP unset) ##########"
  ../bin/deno-mask run -A e6_install_cost.mjs
  echo "################ mask, no-op callback #############################"
  PERMCAP=1 PERMCAP_CB=noop ../bin/deno-mask run -A e6_install_cost.mjs
  echo "################ mask, fast callback ##############################"
  PERMCAP=1 PERMCAP_CB=fast ../bin/deno-mask run -A e6_install_cost.mjs
  echo "################ mask, naive callback (P1's) ######################"
  PERMCAP=1 PERMCAP_CB=naive ../bin/deno-mask run -A e6_install_cost.mjs

  echo "################ nonmask, callback disabled #######################"
  ../bin/deno-nonmask run -A e6_install_cost.mjs
  echo "################ nonmask, no-op callback ##########################"
  PERMCAP=1 PERMCAP_CB=noop ../bin/deno-nonmask run -A e6_install_cost.mjs
  echo "################ nonmask, fast callback ###########################"
  PERMCAP=1 PERMCAP_CB=fast ../bin/deno-nonmask run -A e6_install_cost.mjs
  ;;

startup)
  # NOTE: every row carries an `env` prefix. On macOS the extra /usr/bin/env
  # exec costs ~8.5 ms — more than the whole mechanism — so a row without it
  # is not comparable to a row with it.
  cd app
  echo 'console.log("hi")' > /tmp/permcap_hello.js
  hyperfine -w 5 -m 40 -u millisecond \
    -n 'off            hello' 'env PERMCAP_NONE=1 ../bin/deno-off run /tmp/permcap_hello.js' \
    -n 'accessor       hello' 'env PERMCAP=1 PERMCAP_ACCESSOR=1 ../bin/deno-off run /tmp/permcap_hello.js' \
    -n 'mask (off)     hello' 'env PERMCAP_NONE=1 ../bin/deno-mask run /tmp/permcap_hello.js' \
    -n 'mask (fast)    hello' 'env PERMCAP=1 ../bin/deno-mask run /tmp/permcap_hello.js' \
    -n 'nonmask (off)  hello' 'env PERMCAP_NONE=1 ../bin/deno-nonmask run /tmp/permcap_hello.js' \
    -n 'nonmask (fast) hello' 'env PERMCAP=1 ../bin/deno-nonmask run /tmp/permcap_hello.js'

  echo "### real app: express + lodash + chalk tree ###"
  hyperfine -w 3 -m 20 -u millisecond \
    -n 'off            express' 'env PERMCAP_NONE=1 ../bin/deno-off run -A e3_denial.mjs' \
    -n 'accessor       express' 'env PERMCAP=1 PERMCAP_ACCESSOR=1 ../bin/deno-off run -A e3_denial.mjs' \
    -n 'mask (off)     express' 'env PERMCAP_NONE=1 ../bin/deno-mask run -A e3_denial.mjs' \
    -n 'mask (fast)    express' 'env PERMCAP=1 ../bin/deno-mask run -A e3_denial.mjs' \
    -n 'nonmask (off)  express' 'env PERMCAP_NONE=1 ../bin/deno-nonmask run -A e3_denial.mjs' \
    -n 'nonmask (fast) express' 'env PERMCAP=1 ../bin/deno-nonmask run -A e3_denial.mjs'
  ;;

*)
  echo "usage: $0 {build|bench|startup}" >&2
  exit 1
  ;;
esac
