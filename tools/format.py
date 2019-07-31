#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
from glob import glob
import os
import sys
from third_party import google_env, python_env
from third_party import clang_format_path, third_party_path
from util import root_path, run, find_exts, platform
import argparse

parser = argparse.ArgumentParser()
parser.add_argument("--js", help="only run prettier", action="store_true")
parser.add_argument("--rs", help="only run rustfmt", action="store_true")
parser.add_argument("--py", help="only run yapf", action="store_true")
parser.add_argument("--gn", help="only run gn format", action="store_true")
parser.add_argument("--cc", help="only run clang format", action="store_true")

prettier_path = os.path.join(third_party_path, "node_modules", "prettier",
                             "bin-prettier.js")
tools_path = os.path.join(root_path, "tools")
rustfmt_config = os.path.join(root_path, ".rustfmt.toml")


def main():
    os.chdir(root_path)
    args = parser.parse_args()
    did_fmt = False
    if args.rs:
        rustfmt()
        did_fmt = True
    if args.cc:
        clang_format()
        did_fmt = True
    if args.gn:
        gn_format()
        did_fmt = True
    if args.py:
        yapf()
        did_fmt = True
    if args.js:
        prettier()
        did_fmt = True
    if not did_fmt:
        rustfmt()
        clang_format()
        gn_format()
        yapf()
        prettier()


def qrun(cmd, env=None):
    run(cmd, quiet=True, env=env)


def clang_format():
    print "clang_format"
    qrun([clang_format_path, "-i", "-style", "Google"] +
         find_exts(["core"], [".cc", ".h"]))


def rustfmt():
    print "rustfmt"
    qrun([
        "rustfmt",
        "--config-path",
        rustfmt_config,
    ] + find_exts(["cli", "core", "tools"], [".rs"]))


def gn_format():
    print "gn format"
    for fn in ["BUILD.gn", ".gn"] + find_exts(["build_extra", "cli", "core"],
                                              [".gn", ".gni"]):
        qrun(["third_party/depot_tools/gn", "format", fn], env=google_env())


def yapf():
    print "yapf"
    qrun(
        [sys.executable, "third_party/python_packages/bin/yapf", "-i"] +
        find_exts(["tools", "build_extra"], [".py"], skip=["tools/clang"]),
        env=python_env())


def prettier():
    print "prettier"
    files = find_exts([".github", "js", "tests", "tools", "website", "core"],
                      [".js", ".json", ".ts", ".md"],
                      skip=["tools/clang", "js/deps", "js/gen"])
    qrun(["node", prettier_path, "--write", "--loglevel=error"] +
         ["rollup.config.js"] + files)


if __name__ == '__main__':
    sys.exit(main())
