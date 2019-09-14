#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Does google-lint on c++ files and ts-lint on typescript files

import os
import sys
from util import enable_ansi_colors, find_exts, root_path, run, third_party_path
from third_party import python_env

enable_ansi_colors()

cpplint = os.path.join(third_party_path, "cpplint", "cpplint.py")
eslint = os.path.join(third_party_path, "node_modules", "eslint", "bin",
                      "eslint")

os.chdir(root_path)
run([
    sys.executable, cpplint, "--filter=-build/include_subdir",
    "--repository=core/libdeno", "--extensions=cc,h", "--recursive", "core"
])

run([
    "node", eslint, "--max-warnings=0", "./js/**/*.{ts,js}",
    "./core/**/*.{ts,js}", "./tests/**/*.{ts,js}"
])

run([
    sys.executable, "third_party/python_packages/pylint",
    "--rcfile=third_party/depot_tools/pylintrc"
] + find_exts(["tools", "build_extra"], [".py"], skip=["tools/clang"]),
    env=python_env())
