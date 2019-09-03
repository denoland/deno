#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

# This file just executes its arguments, except that it allows overriding
# environment variables using command-line arguments.

import subprocess
import sys
import os
import re

args = []
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

# Environment variables can be specified on the command line using
# '--env=variable=value' flags. These flags are not passed through to rustc.
# This is useful to set env vars that are normally automatically set by Cargo,
# e.g. CARGO_PKG_NAME, CARGO_PKG_VERSION, OUT_DIR, etc.
for arg in sys.argv[1:]:
    match = re.search('--env=([^=]+)=(.*)', arg)
    if match:
        key, value = match.groups()
        if key == "OUT_DIR":
            # OUT_DIR needs to contain an absolute path.
            value = os.path.abspath(value)
        env[key] = value
    else:
        args.append(arg)

sys.exit(subprocess.call(args, env=env))
