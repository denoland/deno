#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Runs the full test suite.
# This script is wrapper of `cargo test`.
#
# Usage: ./tools/test.py <filter_pattern>
#
# See `./tools/test.py -h` for the available options.
import os
import sys

import http_server
from util import build_path, enable_ansi_colors, rmtree, run


def main():
    deno_dir = os.path.join(build_path(), ".deno_test")
    if os.path.isdir(deno_dir):
        rmtree(deno_dir)
    os.environ["DENO_DIR"] = deno_dir

    enable_ansi_colors()

    cmd = ["cargo", "test", "--locked"] + sys.argv[1:]
    if "--release" in cmd:
        os.environ["DENO_BUILD_MODE"] = "release"
    run(cmd)


if __name__ == '__main__':
    with http_server.spawn():
        main()
