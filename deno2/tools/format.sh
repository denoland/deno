#!/bin/sh
cd `dirname "$0"`/..
clang-format -i -style Google *.cc *.h
gn format BUILD.gn
gn format .gn
yapf -i tools/*.py
