#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

bad_dir=$'bad-\xff'
mkdir -p "$bad_dir"
printf 'module.exports.value = 1;\n' > "$bad_dir/module.cjs"
