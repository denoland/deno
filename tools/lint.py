#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# Does google-lint on c++ files and ts-lint on typescript files

import os
import sys
import argparse
from util import enable_ansi_colors, git_ls_files, git_staged, root_path, run
from util import third_party_path, build_mode, print_command
from third_party import python_env, get_prebuilt_tool_path

cmd_args = None


def get_cmd_args():
    global cmd_args

    if cmd_args:
        return cmd_args

    parser = argparse.ArgumentParser()
    parser.add_argument("--js", help="run dlint", action="store_true")
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
        dlint()
        did_fmt = True
    if args.py:
        pylint()
        did_fmt = True
    if args.rs:
        clippy()
        did_fmt = True

    if not did_fmt:
        dlint()
        pylint()
        clippy()


def dlint():
    executable_path = get_prebuilt_tool_path("dlint")

    # Find all *directories* in the main repo that contain .ts/.js files.
    source_files = get_sources(root_path, [
        "*.js",
        "*.ts",
        ":!:cli/tests/swc_syntax_error.ts",
        ":!:cli/tests/038_checkjs.js",
        ":!:cli/tests/error_008_checkjs.js",
        ":!:std/**/testdata/*",
        ":!:std/**/node_modules/*",
        ":!:cli/bench/node*.js",
        ":!:cli/compilers/wasm_wrap.js",
        ":!:cli/dts/**",
        ":!:cli/tests/encoding/**",
        ":!:cli/tests/error_syntax.js",
        ":!:cli/tests/lint/**",
        ":!:cli/tests/tsc/**",
        ":!:cli/tsc/*typescript.js",
    ])
    if source_files:
        max_command_len = 30000
        pre_command = [executable_path, "run"]
        chunks = [[]]
        cmd_len = len(" ".join(pre_command))
        for f in source_files:
            if cmd_len + len(f) > max_command_len:
                chunks.append([f])
                cmd_len = len(" ".join(pre_command))
            else:
                chunks[-1].append(f)
                cmd_len = cmd_len + len(f) + 1
        for c in chunks:
            print_command("dlint", c)
            run(pre_command + c, shell=False, quiet=True)


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
    print("clippy")
    current_build_mode = build_mode()
    args = ["cargo", "clippy", "--all-targets", "--locked"]
    if current_build_mode != "debug":
        args += ["--release"]
    run(args + ["--", "-D", "clippy::all"], shell=False, quiet=True)


if __name__ == "__main__":
    sys.exit(main())
