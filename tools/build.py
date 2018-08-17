#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import sys
from os.path import join
import third_party
from util import root_path, run, run_output, build_path

third_party.fix_symlinks()

print "DENO_BUILD_PATH:", build_path()
if not os.path.isdir(build_path()):
    print "DENO_BUILD_PATH does not exist. Run tools/setup.py"
    sys.exit(1)
os.chdir(build_path())

ninja_args = sys.argv[1:]

run([third_party.ninja_path] + ninja_args,
    env=third_party.google_env(),
    quiet=True)
