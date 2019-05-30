#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Runs the full test suite.
# Usage: ./tools/test.py out/Debug
import os
import subprocess
import sys
import unittest

from benchmark_test import TestBenchmark
from deno_dir_test import TestDenoDir
from fetch_test import FetchTest
from fmt_test import FmtTest
from integration_tests import TestIntegrations
from repl_test import TestRepl
from setup_test import TestSetup
from unit_tests import JsUnitTests
from util_test import TestUtil

from is_tty_test import TestIsTty
# NOTE: These tests are skipped on Windows
from permission_prompt_test import permission_prompt_tests
from complex_permissions_test import complex_permissions_tests

from http_server import spawn
from util import (DenoTestCase, ColorTextTestRunner, enable_ansi_colors,
                  executable_suffix, run, run_output, rmtree, tests_path,
                  test_args)


class TestTarget(DenoTestCase):
    @staticmethod
    def check_exists(filename):
        if not os.path.exists(filename):
            print "Required target doesn't exist:", filename
            print "Run ./tools/build.py"
            sys.exit(1)

    def test_executable_exists(self):
        self.check_exists(self.deno_exe)

    def _test(self, executable):
        "Test executable runs and exits with code 0."
        bin_file = os.path.join(self.build_dir, executable + executable_suffix)
        self.check_exists(bin_file)
        run([bin_file])

    def test_libdeno(self):
        self._test("libdeno_test")

    def test_cli(self):
        self._test("cli_test")

    def test_core(self):
        self._test("deno_core_test")

    def test_core_http_benchmark(self):
        self._test("deno_core_http_bench_test")

    def test_ts_library_builder(self):
        run([
            "node", "./node_modules/.bin/ts-node", "--project",
            "tools/ts_library_builder/tsconfig.json",
            "tools/ts_library_builder/test.ts"
        ])

    def test_no_color(self):
        t = os.path.join(tests_path, "no_color.js")
        output = run_output([self.deno_exe, "run", t],
                            merge_env={"NO_COLOR": "1"})
        assert output.strip() == "noColor true"
        t = os.path.join(tests_path, "no_color.js")
        output = run_output([self.deno_exe, "run", t])
        assert output.strip() == "noColor false"

    def test_exec_path(self):
        cmd = [self.deno_exe, "run", "tests/exec_path.ts"]
        output = run_output(cmd)
        assert self.deno_exe in output.strip()


def main(argv):
    args = test_args(argv)

    deno_dir = os.path.join(args.build_dir, ".deno_test")
    if os.path.isdir(deno_dir):
        rmtree(deno_dir)
    os.environ["DENO_DIR"] = deno_dir

    enable_ansi_colors()

    with spawn():
        test_cases = [
            TestSetup,
            TestUtil,
            TestTarget,
            JsUnitTests,
            FetchTest,
            FmtTest,
            TestIntegrations,
            TestRepl,
            TestDenoDir,
            TestBenchmark,
        ]
        # These tests are skipped, but to make the test output less noisy
        # we'll avoid triggering them.
        if os.name != 'nt':
            test_cases.append(TestIsTty)
            test_cases += permission_prompt_tests()
            test_cases += complex_permissions_tests()

        suite = unittest.TestSuite([
            unittest.TestLoader().loadTestsFromTestCase(tc)
            for tc in test_cases
        ])

        result = ColorTextTestRunner(
            verbosity=args.verbosity + 1, failfast=args.failfast).run(suite)
        if not result.wasSuccessful():
            sys.exit(1)


if __name__ == '__main__':
    main(sys.argv[1:])
