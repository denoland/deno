#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
import sys
import argparse
from third_party import python_env
from util import git_ls_files, git_staged, third_party_path, root_path
from util import print_command, run

cmd_args = None


def get_cmd_args():
    global cmd_args

    if cmd_args:
        return cmd_args

    parser = argparse.ArgumentParser()
    parser.add_argument("--js", help="run prettier", action="store_true")
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
    script = os.path.join(third_party_path, "node_modules", "prettier",
                          "bin-prettier.js")
    source_files = get_sources(root_path, ["*.js", "*.json", "*.ts", "*.md"])
    if source_files:
        max_command_length = 24000
        while len(source_files) > 0:
            command = ["node", script, "--write", "--loglevel=error", "--"]
            while len(source_files) > 0:
                command.append(source_files.pop())
                if len(" ".join(command)) > max_command_length:
                    break
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
