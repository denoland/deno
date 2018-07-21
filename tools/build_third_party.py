#!/usr/bin/env python
# Only run this script if you are changing Deno's dependencies.

import os
from os.path import join
from util import run, remove_and_symlink

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = join(root_path, "third_party")

try:
    os.makedirs(third_party_path)
except:
    pass
os.chdir(third_party_path)

# Run yarn to install JavaScript dependencies.
remove_and_symlink("../package.json", "package.json")
remove_and_symlink("../yarn.lock", "yarn.lock")
run(["yarn"])
# Run cargo to install Rust dependencies.
run(["cargo", "fetch", "--manifest-path=" + root_path + "/Cargo.toml"],
    envs={'CARGO_HOME': third_party_path + '/rust_crates'})
# Run gclient to install other dependencies.
run(["gclient", "sync", "--reset", "--shallow", "--no-history", "--nohooks"],
    envs={'GCLIENT_FILE': root_path + "/gclient_config.py"})
# TODO(ry) Is it possible to remove these symlinks?
remove_and_symlink("v8/third_party/googletest", "googletest", True)
remove_and_symlink("v8/third_party/jinja2", "jinja2", True)
remove_and_symlink("v8/third_party/llvm-build", "llvm-build", True)
remove_and_symlink("v8/third_party/markupsafe", "markupsafe", True)

# To update the deno_third_party git repo after running this, try the following:
# cd third_party
# find . -type f | grep -v "\.git" | xargs -I% git add -f --no-warn-embedded-repo "%"
