#!/bin/sh
set -e
cd `dirname "$0"`/..
cpplint --filter=-build/include_subdir --repository=src  \
  src/*.cc \
  src/*.h \
  src/include/*.h
