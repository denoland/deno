#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Runs the full test suite.
# Usage: ./tools/test.py out/Debug
import os

import http_server
from util import enable_ansi_colors, rmtree, run
from test_util import parse_test_args


def main():
    args = parse_test_args()

    deno_dir = os.path.join(args.build_dir, ".deno_test")
    if os.path.isdir(deno_dir):
        rmtree(deno_dir)
    os.environ["DENO_DIR"] = deno_dir

    enable_ansi_colors()

    cargo_test = ["cargo", "test", "--locked"]
    if "DENO_BUILD_MODE" in os.environ and \
      os.environ["DENO_BUILD_MODE"] == "release":
        run(cargo_test + ["--release"])
    else:
        run(cargo_test)


if __name__ == '__main__':
    with http_server.spawn():
        main()
