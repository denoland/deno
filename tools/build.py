#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
from __future__ import print_function
import argparse
import os
import sys
import third_party
from util import build_path, enable_ansi_colors, run

parser = argparse.ArgumentParser()
parser.add_argument(
    "--release", help="Use target/release", action="store_true")


def main(argv):
    enable_ansi_colors()

    args, rest_argv = parser.parse_known_args(argv)

    if "DENO_BUILD_MODE" not in os.environ:
        if args.release:
            os.environ["DENO_BUILD_MODE"] = "release"

    ninja_args = rest_argv[1:]
    if not "-C" in ninja_args:
        if not os.path.isdir(build_path()):
            print("Build directory '%s' does not exist." % build_path(),
                  "Run tools/setup.py")
            sys.exit(1)
        ninja_args = ["-C", build_path()] + ninja_args

    run([third_party.ninja_path] + ninja_args,
        env=third_party.google_env(),
        quiet=True)


if __name__ == '__main__':
    sys.exit(main(sys.argv))
