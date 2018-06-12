#!/bin/sh
cd `dirname "$0"`/..
clang-format -i -style Google *.cc *.h include/*.h
gn format BUILD.gn
gn format .gn
yapf -i tools/*.py
prettier --write js/*.ts js/*.js js/*.json
