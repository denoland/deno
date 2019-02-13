#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Does google-lint on c++ files and ts-lint on typescript files

import os
import sys
from util import enable_ansi_colors, find_exts, root_path, run

enable_ansi_colors()

third_party_path = os.path.join(root_path, "third_party")
cpplint = os.path.join(third_party_path, "cpplint", "cpplint.py")
tslint = os.path.join(third_party_path, "node_modules", "tslint", "bin",
                      "tslint")

os.chdir(root_path)
run([
    "python", cpplint, "--filter=-build/include_subdir",
    "--repository=libdeno", "--extensions=cc,h", "--recursive", "libdeno"
])

run(["node", tslint, "-p", ".", "--exclude", "**/gen/**/*.ts"])
run([
    "node", tslint, "./js/**/*_test.ts", "./tests/**/*.ts", "--exclude",
    "**/gen/**/*.ts", "--project", "tsconfig.json"
])

run([sys.executable, "third_party/depot_tools/pylint.py"] +
    find_exts(["tools", "build_extra"], [".py"], skip=["tools/clang"]))
