#!/usr/bin/env python
"""
gn can only run python scripts.
Also Node programs except to be run with cwd = $root_dir/js so it can resolve
node_modules.
"""
import subprocess
import sys
import os

js_path = os.path.dirname(os.path.realpath(__file__))
os.chdir(js_path)
args = ["node"] + sys.argv[1:]
sys.exit(subprocess.call(args))
