#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Given a deno executable, this script executes several integration tests with
# it. The tests are stored in /tests/ and each is specified in a .yaml file
# where a description, command line, and output are specified.  Optionally an
# exit code can be specified.
#
# Usage: integration_tests.py [path to deno executable]
import argparse
import os
import re
import sys
import subprocess
import unittest

from http_server import spawn
from util import (DenoTestCase, ColorTextTestRunner, root_path, tests_path,
                  pattern_match, rmtree, test_main)


def strip_ansi_codes(s):
    ansi_escape = re.compile(r'\x1B\[[0-?]*[ -/]*[@-~]')
    return ansi_escape.sub('', s)


def read_test(file_name):
    with open(file_name, "r") as f:
        test_file = f.read()
    lines = test_file.splitlines()
    test_dict = {}
    for line in lines:
        if line.strip().startswith("#"):
            # skip comments
            continue
        key, value = re.split(r":\s+", line)
        test_dict[key] = value
    return test_dict


def str2bool(v):
    if v == "true":
        return True
    elif v == "false":
        return False
    else:
        raise ValueError("Bad boolean value")


class TestIntegrations(DenoTestCase):
    @classmethod
    def _test(cls, test_filename):
        # Return thunk to test for js file,
        # This is to 'trick' unittest so as to generate these dynamically.
        return lambda self: self.generate(test_filename)

    def generate(self, test_filename):
        test_abs = os.path.join(tests_path, test_filename)
        test = read_test(test_abs)
        exit_code = int(test.get("exit_code", 0))
        args = test.get("args", "").split(" ")
        check_stderr = str2bool(test.get("check_stderr", "false"))
        stderr = subprocess.STDOUT if check_stderr else open(os.devnull, 'w')
        stdin_input = (test.get("input",
                                "").strip().decode("string_escape").replace(
                                    "\r\n", "\n"))
        has_stdin_input = len(stdin_input) > 0

        output_abs = os.path.join(root_path, test.get("output", ""))
        with open(output_abs, 'r') as f:
            expected_out = f.read()
        cmd = [self.deno_exe] + args
        actual_code = 0
        try:
            if has_stdin_input:
                # Provided stdin
                proc = subprocess.Popen(
                    cmd,
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=stderr)
                actual_out, _ = proc.communicate(stdin_input)
                actual_out = actual_out.replace("\r\n", "\n")
            else:
                # No stdin sent
                actual_out = subprocess.check_output(
                    cmd, universal_newlines=True, stderr=stderr)

        except subprocess.CalledProcessError as e:
            actual_code = e.returncode
            actual_out = e.output

        self.assertEqual(exit_code, actual_code)

        actual_out = strip_ansi_codes(actual_out)
        if not pattern_match(expected_out, actual_out):
            # This will always throw since pattern_match failed.
            self.assertEqual(expected_out, actual_out)


# Add a methods for each test file in tests_path.
for fn in sorted(
        filename for filename in os.listdir(tests_path)
        if filename.endswith(".test")):

    t = TestIntegrations._test(fn)
    tn = t.__name__ = "test_" + fn.split(".")[0]
    setattr(TestIntegrations, tn, t)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--filter", help="Run specific tests")
    parser.add_argument(
        "--release", help="Use release build of Deno", action="store_true")
    parser.add_argument("--executable", help="Use external executable of Deno")
    args = parser.parse_args()

    target = "release" if args.release else "debug"
    build_dir = os.environ.get("DENO_BUILD_PATH",
                               os.path.join(root_path, "target", target))

    deno_dir = os.path.join(build_dir, ".deno_test")
    if os.path.isdir(deno_dir):
        rmtree(deno_dir)
    os.environ["DENO_DIR"] = deno_dir

    test_names = [
        test_name for test_name in unittest.TestLoader().getTestCaseNames(
            TestIntegrations) if not args.filter or args.filter in test_name
    ]
    suite = unittest.TestLoader().loadTestsFromNames(
        test_names, module=TestIntegrations)

    with spawn():
        result = ColorTextTestRunner(verbosity=2).run(suite)
        if not result.wasSuccessful():
            sys.exit(1)


if __name__ == "__main__":
    main()
