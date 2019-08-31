#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import subprocess
import sys
import os

d = os.path.dirname(os.path.realpath(__file__))
exe = sys.argv[1]
env = os.environ.copy()
env["CARGO_MANIFEST_DIR"] = d
env["OUT_DIR"] = os.path.dirname(exe)
os.chdir(d)
sys.exit(subprocess.call([exe], env=env))
