#!/bin/sh
cd `dirname "$0"`/..
set -e -v
cpplint --filter=-build/include_subdir --repository=.  *.cc *.h include/*.h
