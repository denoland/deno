#!/bin/sh
set -e
cd `dirname "$0"`/../third_party
gclient sync -j2 --no-history
