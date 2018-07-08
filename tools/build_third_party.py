#!/usr/bin/env python
# This script generates the third party dependencies of deno.
# - Get Depot Tools and make sure it's in your path.
#   http://commondatastorage.googleapis.com/chrome-infra-docs/flat/depot_tools/docs/html/depot_tools_tutorial.html#_setting_up
# - You need yarn installed as well.
#   https://yarnpkg.com/lang/en/docs/install/
# Use //gclient_config.py to modify the git deps.
# Use //js/package.json to modify the npm deps.

import os
from os.path import join
import subprocess
from util import run, remove_and_symlink

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = join(root_path, "third_party")

try:
    os.makedirs(third_party_path)
except:
    pass
os.chdir(third_party_path)
remove_and_symlink(join("..", "gclient_config.py"), ".gclient")
remove_and_symlink(join("..", "package.json"), "package.json")
remove_and_symlink(join("..", "yarn.lock"), "yarn.lock")
remove_and_symlink(join("v8", "third_party", "googletest"), "googletest")
remove_and_symlink(join("v8", "third_party", "jinja2"), "jinja2")
remove_and_symlink(join("v8", "third_party", "llvm-build"), "llvm-build")
remove_and_symlink(join("v8", "third_party", "markupsafe"), "markupsafe")
run(["gclient", "sync", "--no-history"])
run(["yarn"])
