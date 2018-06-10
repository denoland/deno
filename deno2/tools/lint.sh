#!/bin/sh
cd `dirname "$0"`/..
set -e -v
cpplint --repository=.  *.cc *.h
