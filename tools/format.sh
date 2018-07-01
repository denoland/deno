#!/bin/sh
set -e
cd `dirname "$0"`/..
clang-format -i -style Google src/*.cc src/*.h src/include/*.h

gn format BUILD.gn
gn format deno.gni
gn format .gn

yapf -i src/js/*.py
prettier --write \
  src/js/deno.d.ts \
  src/js/main.ts \
  src/js/mock_runtime.js \
  src/js/tsconfig.json
# Do not format these.
#  src/js/msg.pb.js
#  src/js/msg.pb.d.ts

rustfmt --write-mode overwrite src/*.rs
