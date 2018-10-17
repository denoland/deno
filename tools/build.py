#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
from __future__ import print_function
import os
import sys
import third_party
from util import build_path, enable_ansi_colors, run


def main(argv):
    enable_ansi_colors()

    third_party.fix_symlinks()

    ninja_args = argv[1:]
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
