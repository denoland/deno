#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import sys
import argparse
from third_party import google_env, python_env
from util import git_ls_files, third_party_path, root_path, run


def main():
    os.chdir(root_path)

    parser = argparse.ArgumentParser()
    parser.add_argument("--cc", help="run clang-format", action="store_true")
    parser.add_argument("--gn", help="run gn format", action="store_true")
    parser.add_argument("--js", help="run prettier", action="store_true")
    parser.add_argument("--py", help="run yapf", action="store_true")
    parser.add_argument("--rs", help="run rustfmt", action="store_true")
    args = parser.parse_args()

    did_fmt = False
    if args.cc:
        clang_format()
        did_fmt = True
    if args.gn:
        gn_format()
        did_fmt = True
    if args.js:
        prettier()
        did_fmt = True
    if args.py:
        yapf()
        did_fmt = True
    if args.rs:
        rustfmt()
        did_fmt = True

    if not did_fmt:
        clang_format()
        gn_format()
        prettier()
        yapf()
        rustfmt()


def clang_format():
    print "clang_format"
    exe = os.path.join(third_party_path, "depot_tools", "clang-format")
    source_files = git_ls_files(root_path, ["*.cc", "*.h"])
    run([exe, "-i", "-style", "Google", "--"] + source_files,
        env=google_env(),
        quiet=True)


def gn_format():
    print "gn format"
    exe = os.path.join(third_party_path, "depot_tools", "gn")
    source_files = git_ls_files(root_path, ["*.gn", "*.gni"])
    run([exe, "format", "--"] + source_files, env=google_env(), quiet=True)


def prettier():
    print "prettier"
    script = os.path.join(third_party_path, "node_modules", "prettier",
                          "bin-prettier.js")
    source_files = git_ls_files(root_path, ["*.js", "*.json", "*.ts", "*.md"])
    run(["node", script, "--write", "--loglevel=error", "--"] + source_files,
        shell=False,
        quiet=True)


def yapf():
    print "yapf"
    script = os.path.join(third_party_path, "python_packages", "bin", "yapf")
    source_files = git_ls_files(root_path, ["*.py"])
    run([sys.executable, script, "-i", "--"] + source_files,
        env=python_env(),
        shell=False,
        quiet=True)


def rustfmt():
    print "rustfmt"
    config_file = os.path.join(root_path, ".rustfmt.toml")
    source_files = git_ls_files(root_path, ["*.rs"])
    run([
        "rustfmt",
        "--config-path=" + config_file,
        "--",
    ] + source_files,
        shell=False,
        quiet=True)


if __name__ == "__main__":
    sys.exit(main())
