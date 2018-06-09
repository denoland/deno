#!/bin/sh
cd `dirname "$0"`/..
set -e
cpplint *.cc *.h
