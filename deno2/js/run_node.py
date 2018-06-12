#!/usr/bin/env python
"""
gn can only run python scripts.
"""
import subprocess
import sys
import os


js_path = os.path.dirname(os.path.realpath(__file__))
node_modules_path = os.path.join(js_path, "node_modules")

# root_out_dir
if not os.path.exists("node_modules"):
  os.symlink(node_modules_path, "node_modules")

args = ["node"] + sys.argv[1:]
sys.exit(subprocess.call(args))
