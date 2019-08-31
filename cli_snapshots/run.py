#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# This script is to execute build.rs during the GN build. See BUILD.gn.
import subprocess
import sys
import os

d = os.path.dirname(os.path.realpath(__file__))
exe = sys.argv[1]
env = os.environ.copy()
env["CARGO_MANIFEST_DIR"] = d
env["OUT_DIR"] = os.path.dirname(exe)
# To match the behavior of cargo, we need to cd into this directory.
os.chdir(d)
sys.exit(subprocess.call([exe], env=env))
