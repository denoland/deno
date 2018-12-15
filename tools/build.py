#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.

# This is a simple wrapper around ninja.
# Do not add futher complexity to this script.

from __future__ import print_function
import os
import sys
import third_party
from util import enable_ansi_colors, run


def main(argv):
    enable_ansi_colors()

    third_party.fix_symlinks()

    ninja_args = argv[1:]
    if not "-C" in ninja_args:
        gn_out = "target/debug"
        if not os.path.isdir(gn_out):
            print("Build directory '%s' does not exist." % gn_out,
                  "Run tools/setup.py")
            sys.exit(1)
        ninja_args = ["-C", gn_out] + ninja_args

    run([third_party.ninja_path] + ninja_args,
        env=third_party.google_env(),
        quiet=True)


if __name__ == '__main__':
    sys.exit(main(sys.argv))
