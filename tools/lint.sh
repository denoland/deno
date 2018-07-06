#!/bin/sh
# TODO(ry) Rewrite this script in python for portability to Windows.
set -e
cd `dirname "$0"`/..
./third_party/cpplint/cpplint.py \
  --filter=-build/include_subdir \
  --repository=src  \
  src/*.cc \
  src/*.h
node third_party/node_modules/.bin/tslint -p . \
  --exclude js/msg_generated.ts
