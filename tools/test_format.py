#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# This program fails if ./tools/format.ts changes any files.

import os
import sys
import util
import sys
import subprocess
from distutils.spawn import find_executable


def lookup_deno_path():
    deno_exe = "deno" + util.executable_suffix
    release_deno = os.path.join(util.root_path, "target", "release", deno_exe)
    debug_deno = os.path.join(util.root_path, "target", "debug", deno_exe)

    if os.path.exists(release_deno):
        return release_deno
    if os.path.exists(debug_deno):
        return debug_deno

    return find_executable("deno")


def main():
    deno_path = lookup_deno_path()

    if not deno_path:
        print "No available deno executable."
        sys.exit(1)

    util.run([deno_path, "--allow-read", "--allow-run", "tools/format.ts"])
    output = util.run_output(
        ["git", "status", "-uno", "--porcelain", "--ignore-submodules"])
    if len(output) > 0:
        print "Run tools/format.ts "
        print output
        sys.exit(1)


if __name__ == '__main__':
    main()
