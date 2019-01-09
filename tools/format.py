#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
from glob import glob
import os
import sys
from third_party import fix_symlinks, google_env, python_env
from third_party import clang_format_path, third_party_path
from util import root_path, run, find_exts, platform

fix_symlinks()

prettier = os.path.join(third_party_path, "node_modules", "prettier",
                        "bin-prettier.js")
tools_path = os.path.join(root_path, "tools")
rustfmt_config = os.path.join(tools_path, "rustfmt.toml")

os.chdir(root_path)


def qrun(cmd, env=None):
    run(cmd, quiet=True, env=env)


print "clang_format"
qrun([clang_format_path, "-i", "-style", "Google"] +
     find_exts(["libdeno"], [".cc", ".h"]))

print "gn format"
for fn in ["BUILD.gn", ".gn"] + find_exts(["build_extra", "libdeno"],
                                          [".gn", ".gni"]):
    qrun(["third_party/depot_tools/gn", "format", fn], env=google_env())

print "yapf"
qrun(
    [sys.executable, "third_party/python_packages/bin/yapf", "-i"] + find_exts(
        ["tools", "build_extra"], [".py"], skip=["tools/clang"]),
    env=python_env())

print "prettier"
qrun(["node", prettier, "--write", "--loglevel=error"] + ["rollup.config.js"] +
     glob("*.json") + glob("*.md") +
     find_exts([".github", "js", "tests", "tools", "website"],
               [".js", ".json", ".ts", ".md"],
               skip=["tools/clang", "js/deps"]))

print "rustfmt"
qrun([
    "third_party/rustfmt/" + platform() +
    "/rustfmt", "--config-path", rustfmt_config, "build.rs"
] + find_exts(["src"], [".rs"]))
