#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# Given a deno executable, this script executes several integration tests with
# it. The tests are stored in /tests/ and each is specified in a .yaml file
# where a description, command line, and output are specified.  Optionally an
# exit code can be specified.
#
# Usage: integration_tests.py [path to deno executable]
import os
import re
import sys
import subprocess
from util import root_path, tests_path, pattern_match, green_ok, red_failed


def read_test(file_name):
    with open(file_name, "r") as f:
        test_file = f.read()
    lines = test_file.splitlines()
    test_dict = {}
    for line in lines:
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


def integration_tests(deno_executable):
    assert os.path.isfile(deno_executable)
    tests = sorted([
        filename for filename in os.listdir(tests_path)
        if filename.endswith(".test")
    ])
    assert len(tests) > 0
    for test_filename in tests:
        test_abs = os.path.join(tests_path, test_filename)
        test = read_test(test_abs)
        exit_code = int(test.get("exit_code", 0))
        args = test.get("args", "").split(" ")

        check_stderr = str2bool(test.get("check_stderr", "false"))
        stderr = subprocess.STDOUT if check_stderr else None

        output_abs = os.path.join(root_path, test.get("output", ""))
        with open(output_abs, 'r') as f:
            expected_out = f.read()
        cmd = [deno_executable] + args
        print "test %s" % (test_filename)
        print " ".join(cmd)
        actual_code = 0
        try:
            actual_out = subprocess.check_output(
                cmd, universal_newlines=True, stderr=stderr)
        except subprocess.CalledProcessError as e:
            actual_code = e.returncode
            actual_out = e.output

        if exit_code != actual_code:
            print "... " + red_failed()
            print "Expected exit code %d but got %d" % (exit_code, actual_code)
            print "Output:"
            print actual_out
            sys.exit(1)

        if pattern_match(expected_out, actual_out) != True:
            print "... " + red_failed()
            print "Expected output does not match actual."
            print "Expected output: \n" + expected_out
            print "Actual output:   \n" + actual_out
            sys.exit(1)

        print "... " + green_ok()


def main(argv):
    integration_tests(argv[1])


if __name__ == "__main__":
    sys.exit(main(sys.argv))
