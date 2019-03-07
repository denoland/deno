#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
"""
gn can only run python scripts. This launches a subprocess Node process.
The working dir of this program is out/Debug/ (AKA root_build_dir)
Before running node, we symlink js/node_modules to out/Debug/node_modules.
"""
import sys
from os import path
from util import symlink, root_path, run

if not path.exists("node_modules"):
    target_abs = path.join(root_path, "third_party/node_modules")
    target_rel = path.relpath(target_abs)
    symlink(target_rel, "node_modules", True)

run(["node"] + sys.argv[1:], quiet=True)
