#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Runs the full test suite.
# Usage: ./tools/test.py out/Debug
import os

from benchmark_test import TestBenchmark
from deno_dir_test import TestDenoDir
from fetch_test import TestFetch
from fmt_test import TestFmt
from repl_test import TestRepl
from setup_test import TestSetup
from target_test import TestTarget
from unit_tests import JsUnitTests
from util_test import TestUtil
# NOTE: These tests are skipped on Windows
from is_tty_test import TestIsTty
from permission_prompt_test import permission_prompt_tests
from complex_permissions_test import complex_permissions_tests

import http_server
from util import (enable_ansi_colors, build_path, RESET, FG_RED, FG_GREEN,
                  executable_suffix, rmtree, tests_path)
from test_util import parse_test_args, run_tests


def main():
    args = parse_test_args()

    deno_dir = os.path.join(args.build_dir, ".deno_test")
    if os.path.isdir(deno_dir):
        rmtree(deno_dir)
    os.environ["DENO_DIR"] = deno_dir

    enable_ansi_colors()

    test_cases = [
        TestSetup,
        TestUtil,
        TestTarget,
        JsUnitTests,
        TestFetch,
        TestRepl,
        TestDenoDir,
        TestBenchmark,
        TestIsTty,
    ]
    test_cases += permission_prompt_tests()
    test_cases += complex_permissions_tests()
    # It is very slow, so do TestFmt at the end.
    test_cases += [TestFmt]

    with http_server.spawn():
        run_tests(test_cases)


if __name__ == '__main__':
    main()
