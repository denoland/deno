#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# This file just executes its arguments, except that also adds GN_OUT_DIR and
# CARGO_PKG_VERSION to the environ. This is for compatibility with cargo.
import subprocess
import sys
import os
import re

# This is for src/msg.rs to know where to find msg_generated.rs.
# When building with Cargo this variable is set by build.rs.
os.environ["GN_OUT_DIR"] = os.path.abspath(".")
assert os.path.isdir(os.environ["GN_OUT_DIR"])

# Set the CARGO_PKG_VERSION env variable if provided as an argument
# When building with Cargo this variable is set automatically
args = sys.argv[1:]
for i, arg in enumerate(args):
    match = re.search('--cargo-pkg-version="?([^"]*)"?', arg)
    if match:
        os.environ["CARGO_PKG_VERSION"] = match.group(1)
        del args[i]
        break
# Set the custom env variables if provided as an argument
for i, arg in enumerate(args):
    match = re.search('--custom-envs="?([^"]*)"?', arg)
    if match:
        env_vars = match.group(1).split(",")
        for env_var in env_vars:
            env_var = env_var.strip()
            if len(env_var) > 0:
                env_args = env_var.split("=")
                os.environ[env_args[0]] = env_args[1]
        del args[i]
        break

sys.exit(subprocess.call(args))
