#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# Runs the full test suite.
# Usage: ./tools/test.py out/Debug
import argparse
import contextlib
import os
import sys
import unittest

from util import (enable_ansi_colors, build_path, RESET, FG_RED, FG_GREEN,
                  executable_suffix, rmtree, tests_path)


class DenoTestCase(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        args = parse_test_args()

        cls.build_dir = args.build_dir
        cls.deno_exe = args.executable


# overload the test result class
class ColorTextTestResult(unittest.TextTestResult):
    @contextlib.contextmanager
    def color(self, code):
        self.stream.write(code)
        try:
            yield
        finally:
            self.stream.write(RESET)

    def getDescription(self, test):
        name = str(test)
        if name.startswith("test_"):
            name = name[5:]
        return name

    def addSuccess(self, test):
        with self.color(FG_GREEN):
            super(ColorTextTestResult, self).addSuccess(test)

    def addError(self, test, err):
        with self.color(FG_RED):
            super(ColorTextTestResult, self).addError(test, err)

    def addFailure(self, test, err):
        with self.color(FG_RED):
            super(ColorTextTestResult, self).addFailure(test, err)


class ColorTextTestRunner(unittest.TextTestRunner):
    resultclass = ColorTextTestResult


def create_test_arg_parser():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        '--failfast', '-f', action='store_true', help='Stop on first failure')
    parser.add_argument(
        '--verbose', '-v', action='store_true', help='Verbose output')
    parser.add_argument("--executable", help="Use external executable of Deno")
    parser.add_argument(
        '--release',
        action='store_true',
        help='Test against release executable')
    parser.add_argument(
        '--pattern', '-p', help='Run tests that match provided pattern')
    parser.add_argument(
        '--build-dir', dest="build_dir", help='Deno build directory')
    return parser


TestArgParser = create_test_arg_parser()


def parse_test_args(argv=None):
    if argv is None:
        argv = sys.argv[1:]

    args = TestArgParser.parse_args(argv)

    if args.executable and args.release:
        raise argparse.ArgumentError(
            None, "Path to executable is inferred from "
            "--release, cannot provide both.")

    if not args.build_dir:
        args.build_dir = build_path()

    if not args.executable:
        args.executable = os.path.join(args.build_dir,
                                       "deno" + executable_suffix)

    if not os.path.isfile(args.executable):
        raise argparse.ArgumentError(
            None, "deno executable not found at {}".format(args.executable))

    return args


def filter_test_suite(suite, pattern):
    filtered_tests = []

    for test_case in suite:
        if isinstance(test_case, unittest.TestSuite):
            filtered_tests += filter_test_suite(test_case, pattern)
        else:
            if pattern in str(test_case):
                filtered_tests.append(test_case)

    return filtered_tests


def run_tests(test_cases=None):
    args = parse_test_args()

    loader = unittest.TestLoader()

    # if suite was not explicitly passed load test
    # cases from calling module
    if test_cases is None:
        import __main__
        suite = loader.loadTestsFromModule(__main__)
    else:
        suite = unittest.TestSuite()
        for test_case in test_cases:
            suite.addTests(loader.loadTestsFromTestCase(test_case))

    if args.pattern:
        filtered_tests = filter_test_suite(suite, args.pattern)
        suite = unittest.TestSuite(filtered_tests)

    runner = ColorTextTestRunner(
        verbosity=args.verbose + 2, failfast=args.failfast)

    result = runner.run(suite)
    if not result.wasSuccessful():
        sys.exit(1)
