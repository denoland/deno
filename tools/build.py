#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
from __future__ import print_function
import argparse
import os
import sys
import third_party
from util import build_path, enable_ansi_colors, run

enable_ansi_colors()

third_party.fix_symlinks()

parser = argparse.ArgumentParser()
build_option_group = parser.add_mutually_exclusive_group()
build_option_group.add_argument("--debug", action="store_true")
build_option_group.add_argument("--release", action="store_true")
# This makes -C option not allow to use --debug or --release together.
build_option_group.add_argument("-C", action="store", nargs=1, metavar="path")
build_option, ninja_args = parser.parse_known_args()

if build_option.C != None:
    ninja_args += ["-C"] + build_option.C
else:
    if build_option.release:
        build_mode = "release"
    else:
        build_mode = "debug"

    if not os.path.isdir(build_path(build_mode)):
        print("Build directory '%s' does not exist." % build_path(build_mode),
              "Run tools/setup.py")
        sys.exit(1)
    ninja_args += ["-C", build_path(build_mode)]

run([third_party.ninja_path] + ninja_args,
    env=third_party.google_env(),
    quiet=True)
