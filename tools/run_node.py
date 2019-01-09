#!/usr/bin/env python
"""
gn can only run python scripts. This launches a subprocess Node process.
The working dir of this program is out/Debug/ (AKA root_build_dir)
Before running node, we symlink js/node_modules to out/Debug/node_modules.
"""
import subprocess
import sys
import os
import util

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
tools_path = os.path.join(root_path, "tools")
third_party_path = os.path.join(root_path, "third_party")
target_abs = os.path.join(third_party_path, "node_modules")
target_rel = os.path.relpath(target_abs)

util.remove_and_symlink(target_rel, "node_modules", True)
util.run(["node"] + sys.argv[1:], quiet=True)
