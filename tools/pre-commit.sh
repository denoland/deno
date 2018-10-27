#!/bin/sh
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# The script for git pre-commit hook.
# This script formats the source code and adds them back to staging.
staged=$(git diff --cached --name-only --diff-filter=ACM | tr '\n' ' ')
[ -z "$staged" ] && exit 0
echo Formatting files
./tools/format.py
echo "$staged" | xargs git add
exit 0
