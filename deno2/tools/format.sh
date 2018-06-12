#!/bin/sh
cd `dirname "$0"`/..
clang-format -i -style Google *.cc *.h include/*.h
gn format BUILD.gn
gn format .gn
yapf -i tools/*.py
prettier --write \
	js/deno.d.ts \
  js/main.ts \
  js/mock_runtime.js \
  js/package.json \
  js/tsconfig.json
# Do not format these. 
#  js/msg.pb.js
#  js/msg.pb.d.ts
