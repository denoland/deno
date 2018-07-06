#!/bin/sh
set -e
cd `dirname "$0"`/..
clang-format -i -style Google src/*.cc src/*.h src/include/*.h

gn format BUILD.gn
gn format deno.gni
gn format rust.gni
gn format .gn

yapf -i js/*.py
yapf -i tools/*.py

prettier --write \
  js/deno.d.ts \
  js/main.ts \
  js/mock_runtime.js \
  tsconfig.json
# Do not format these.
#  js/msg_generated.ts
#  js/flatbuffers.js

rustfmt --write-mode overwrite src/*.rs
