#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# Does google-lint on c++ files and ts-lint on typescript files

import os
import sys
import argparse
from util import enable_ansi_colors, git_ls_files, root_path, run
from util import third_party_path, build_mode
from third_party import python_env


def main():
    enable_ansi_colors()
    os.chdir(root_path)

    parser = argparse.ArgumentParser()
    parser.add_argument("--js", help="run eslint", action="store_true")
    parser.add_argument("--py", help="run pylint", action="store_true")
    parser.add_argument("--rs", help="run clippy", action="store_true")
    args = parser.parse_args()

    did_fmt = False
    if args.js:
        eslint()
        did_fmt = True
    if args.py:
        pylint()
        did_fmt = True
    if args.rs:
        clippy()
        did_fmt = True

    if not did_fmt:
        eslint()
        pylint()
        clippy()


def eslint():
    print "eslint"
    script = os.path.join(third_party_path, "node_modules", "eslint", "bin",
                          "eslint")
    # Find all *directories* in the main repo that contain .ts/.js files.
    source_files = git_ls_files(root_path, [
        "*.js", "*.ts", ":!:std/**/testdata/*", ":!:std/**/node_modules/*",
        ":!:cli/compilers/*"
    ])
    source_dirs = set([os.path.dirname(f) for f in source_files])
    # Within the source dirs, eslint does its own globbing, taking into account
    # the exclusion rules listed in '.eslintignore'.
    source_globs = ["%s/*.{js,ts}" % d for d in source_dirs]
    # Set NODE_PATH so we don't have to maintain a symlink in root_path.
    env = os.environ.copy()
    env["NODE_PATH"] = os.path.join(root_path, "third_party", "node_modules")
    run(["node", script, "--max-warnings=0", "--"] + source_globs,
        shell=False,
        env=env,
        quiet=True)


def pylint():
    print "pylint"
    script = os.path.join(third_party_path, "python_packages", "pylint")
    rcfile = os.path.join(root_path, "tools", "pylintrc")
    source_files = git_ls_files(root_path, ["*.py"])
    run([sys.executable, script, "--rcfile=" + rcfile, "--"] + source_files,
        env=python_env(),
        shell=False,
        quiet=True)


def clippy():
    print "clippy"
    current_build_mode = build_mode()
    args = ["cargo", "clippy", "--all-targets", "--locked"]
    if current_build_mode != "debug":
        args += ["--release"]
    run(args + ["--", "-D", "clippy::all"], shell=False, quiet=True)


if __name__ == "__main__":
    sys.exit(main())
