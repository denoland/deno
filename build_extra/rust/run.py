#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# This file just executes its arguments, except that also adds GN_OUT_DIR and
# CARGO_PKG_VERSION to the environ. This is for compatibility with cargo.
import subprocess
import sys
import os
import re

env = os.environ.copy()

if sys.platform == 'win32':
    # On Windows, when gn is setting up the build toolchain, it produces a set
    # of environment variables that are required to invoke the right build
    # toolchain. We need to load those environment variables here too in order
    # for rustc to be able to successfully invoke the linker tool.
    # The file is in 'windows environment block' format, which contains
    # multiple 'key=value' pairs, separated by '\0' bytes, and terminated by
    # two '\0' bytes at the end.
    gn_env_pairs = open("environment.x64").read()[:-2].split('\0')
    gn_env = dict([pair.split('=', 1) for pair in gn_env_pairs])
    env.update(gn_env)

# This is for src/msg.rs to know where to find msg_generated.rs.
# When building with Cargo this variable is set by build.rs.
env["GN_OUT_DIR"] = os.path.abspath(".")
assert os.path.isdir(env["GN_OUT_DIR"])

# Set the CARGO_PKG_VERSION env variable if provided as an argument
# When building with Cargo this variable is set automatically
args = sys.argv[1:]
for i, arg in enumerate(args):
    match = re.search('--cargo-pkg-version="?([^"]*)"?', arg)
    if match:
        env["CARGO_PKG_VERSION"] = match.group(1)
        del args[i]
        break

sys.exit(subprocess.call(args, env=env))
