#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# This file just executes its arguments, except that also adds GN_OUT_DIR and
# CARGO_PKG_VERSION to the environ. This is for compatibility with cargo.
import subprocess
import sys
import os
import re

exe = sys.argv[1]
d = sys.argv[2]
root_out_dir = sys.argv[3]

assert os.path.exists(exe)

env = os.environ.copy()
env["CARGO_MANIFEST_DIR"] = os.path.abspath(d)
env["OUT_DIR"] = root_out_dir

os.chdir(d)
sys.exit(subprocess.call([exe, "foo"], env=env))
