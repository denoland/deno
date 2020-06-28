#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# Does google-lint on c++ files and ts-lint on typescript files

import os
import sys
import argparse
from util import enable_ansi_colors, git_ls_files, git_staged, root_path, run
from util import third_party_path, build_mode, print_command
from third_party import python_env

cmd_args = None


def get_cmd_args():
    global cmd_args

    if cmd_args:
        return cmd_args

    parser = argparse.ArgumentParser()
    parser.add_argument("--js", help="run eslint", action="store_true")
    parser.add_argument("--py", help="run pylint", action="store_true")
    parser.add_argument("--rs", help="run clippy", action="store_true")
    parser.add_argument(
        "--staged", help="run only on staged files", action="store_true")
    cmd_args = parser.parse_args()
    return cmd_args


def get_sources(*args):
    getter = git_staged if get_cmd_args().staged else git_ls_files
    return getter(*args)


def main():
    enable_ansi_colors()
    os.chdir(root_path)

    args = get_cmd_args()

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
    script = os.path.join(third_party_path, "node_modules", "eslint", "bin",
                          "eslint")
    # Find all *directories* in the main repo that contain .ts/.js files.
    source_files = get_sources(root_path, [
        "*.js",
        "*.ts",
        ":!:cli/tests/swc_syntax_error.ts",
        ":!:std/**/testdata/*",
        ":!:std/**/node_modules/*",
        ":!:cli/compilers/wasm_wrap.js",
        ":!:cli/tests/error_syntax.js",
        ":!:cli/tests/lint/**",
    ])
    if source_files:
        print_command("eslint", source_files)
        # Set NODE_PATH so we don't have to maintain a symlink in root_path.
        env = os.environ.copy()
        env["NODE_PATH"] = os.path.join(root_path, "third_party",
                                        "node_modules")
        run(["node", script, "--max-warnings=0", "--"] + source_files,
            shell=False,
            env=env,
            quiet=True)


def pylint():
    script = os.path.join(third_party_path, "python_packages", "pylint")
    rcfile = os.path.join(root_path, "tools", "pylintrc")
    msg_template = "{path}({line}:{column}) {category}: {msg} ({symbol})"
    source_files = get_sources(root_path, ["*.py"])
    if source_files:
        print_command("pylint", source_files)
        run([
            sys.executable, script, "--rcfile=" + rcfile,
            "--msg-template=" + msg_template, "--"
        ] + source_files,
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
