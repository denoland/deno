#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
import sys
import argparse
from third_party import python_env, get_prebuilt_tool_path
from util import git_ls_files, git_staged, third_party_path, root_path
from util import print_command, run

cmd_args = None


def get_cmd_args():
    global cmd_args

    if cmd_args:
        return cmd_args

    parser = argparse.ArgumentParser()
    parser.add_argument("--js", help="run dprint", action="store_true")
    parser.add_argument("--py", help="run yapf", action="store_true")
    parser.add_argument("--rs", help="run rustfmt", action="store_true")
    parser.add_argument(
        "--staged", help="run only on staged files", action="store_true")
    cmd_args = parser.parse_args()
    return cmd_args


def get_sources(*args):
    getter = git_staged if get_cmd_args().staged else git_ls_files
    return getter(*args)


def main():
    os.chdir(root_path)

    args = get_cmd_args()

    did_fmt = False
    if args.js:
        dprint()
        did_fmt = True
    if args.py:
        yapf()
        did_fmt = True
    if args.rs:
        rustfmt()
        did_fmt = True

    if not did_fmt:
        dprint()
        yapf()
        rustfmt()


def dprint():
    executable_path = get_prebuilt_tool_path("dprint")
    command = [executable_path, "fmt"]
    run(command, shell=False, quiet=True)


def yapf():
    script = os.path.join(third_party_path, "python_packages", "bin", "yapf")
    source_files = get_sources(root_path, ["*.py"])
    if source_files:
        print_command("yapf", source_files)
        run([sys.executable, script, "-i", "--style=pep8", "--"] +
            source_files,
            env=python_env(),
            shell=False,
            quiet=True)


def rustfmt():
    config_file = os.path.join(root_path, ".rustfmt.toml")
    source_files = get_sources(root_path, ["*.rs"])
    if source_files:
        print_command("rustfmt", source_files)
        run([
            "rustfmt",
            "--config-path=" + config_file,
            "--",
        ] + source_files,
            shell=False,
            quiet=True)


if __name__ == "__main__":
    sys.exit(main())
