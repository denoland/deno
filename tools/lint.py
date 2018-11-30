#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# Does google-lint on c++ files and ts-lint on typescript files

import os
import sys
from util import enable_ansi_colors, run, find_exts

enable_ansi_colors()

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = os.path.join(root_path, "third_party")
cpplint = os.path.join(third_party_path, "cpplint", "cpplint.py")
tslint = os.path.join(third_party_path, "node_modules", "tslint", "bin",
                      "tslint")

os.chdir(root_path)
run([
    "python", cpplint, "--filter=-build/include_subdir", "--repository=src",
    "--extensions=cc,h", "--recursive", "src/."
])

run(["node", tslint, "-p", ".", "--exclude", "**/gen/**/*.ts"])
run([
    "node", tslint, "./js/**/*_test.ts", "./tests/**/*.ts", "--exclude",
    "**/gen/**/*.ts", "--project", "tsconfig.json"
])

run([sys.executable, "third_party/depot_tools/pylint.py"] +
    find_exts(["tools", "build_extra"], [".py"], skip=["tools/clang"]))
