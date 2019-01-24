#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
"""
gn can only run python scripts. This launches a subprocess Node process.
The working dir of this program is out/Debug/ (AKA root_build_dir)
Before running node, we symlink js/node_modules to out/Debug/node_modules.
"""
import subprocess
import sys
import os
from util import remove_and_symlink, root_path, run

tools_path = os.path.join(root_path, "tools")
third_party_path = os.path.join(root_path, "third_party")
target_abs = os.path.join(third_party_path, "node_modules")
target_rel = os.path.relpath(target_abs)

remove_and_symlink(target_rel, "node_modules", True)
run(["node"] + sys.argv[1:], quiet=True)
