#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
import sys
import argparse
from third_party import python_env
from util import git_ls_files, third_party_path, root_path, run


def main():
    os.chdir(root_path)

    parser = argparse.ArgumentParser()
    parser.add_argument("--js", help="run prettier", action="store_true")
    parser.add_argument("--py", help="run yapf", action="store_true")
    parser.add_argument("--rs", help="run rustfmt", action="store_true")
    args = parser.parse_args()

    did_fmt = False
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
        prettier()
        yapf()
        rustfmt()


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
