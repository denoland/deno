#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
from glob import glob
import os
from third_party import third_party_path, fix_symlinks, google_env, clang_format_path
from util import root_path, run, find_exts, platform

fix_symlinks()

prettier = os.path.join(third_party_path, "node_modules", "prettier",
                        "bin-prettier.js")
tools_path = os.path.join(root_path, "tools")
rustfmt_config = os.path.join(tools_path, "rustfmt.toml")

os.chdir(root_path)

run([clang_format_path, "-i", "-style", "Google"] +
    find_exts("libdeno", ".cc", ".h"))

for fn in ["BUILD.gn", ".gn"] + find_exts("build_extra", ".gn", ".gni"):
    run(["third_party/depot_tools/gn", "format", fn], env=google_env())

# We use `glob()` instead of `find_exts()` in the tools directory, because:
#   * On Windows, `os.walk()` (called by `find_exts()`) follows symlinks.
#   * The tools directory contains a symlink 'clang', pointing at the directory
#     'third_party/v8/tools/clang', which contains many .py files.
#   * These third party python files shouldn't be formatted.
#   * The tools directory has no subdirectories, so `glob()` is sufficient.
# TODO(ry) Install yapf in third_party.
run(["yapf", "-i"] + glob("tools/*.py") + find_exts("build_extra", ".py"))

# yapf: disable
run(["node", prettier, "--write"] +
    ["rollup.config.js"] + glob("*.json") + glob("*.md") +
    find_exts(".github/", ".md") +
    find_exts("js/", ".js", ".ts", ".md") +
    find_exts("tests/", ".js", ".ts", ".md") +
    find_exts("tools/", ".js", ".json", ".ts", ".md") +
    find_exts("website/", ".js", ".ts", ".md"))
# yapf: enable

run([
    "third_party/rustfmt/" + platform() +
    "/rustfmt", "--config-path", rustfmt_config
] + find_exts("src/", ".rs"))
