#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
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
import http_server
import argparse
from util import root_path, tests_path, pattern_match, \
                 green_ok, red_failed, rmtree, executable_suffix


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


def integration_tests(deno_exe, test_filter = None):
    assert os.path.isfile(deno_exe)
    tests = sorted([
        filename for filename in os.listdir(tests_path)
        if filename.endswith(".test")
    ])
    assert len(tests) > 0
    for test_filename in tests:
        if test_filter and test_filter not in test_filename:
            continue

        test_abs = os.path.join(tests_path, test_filename)
        test = read_test(test_abs)
        exit_code = int(test.get("exit_code", 0))
        args = test.get("args", "").split(" ")

        check_stderr = str2bool(test.get("check_stderr", "false"))

        stderr = subprocess.STDOUT if check_stderr else open(os.devnull, 'w')

        output_abs = os.path.join(root_path, test.get("output", ""))
        with open(output_abs, 'r') as f:
            expected_out = f.read()
        cmd = [deno_exe] + args
        sys.stdout.write("tests/%s ... " % (test_filename))
        sys.stdout.flush()
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

        actual_out = strip_ansi_codes(actual_out)

        if pattern_match(expected_out, actual_out) != True:
            print red_failed()
            print "Expected output does not match actual."
            print "Expected output: \n" + expected_out
            print "Actual output:   \n" + actual_out
            sys.exit(1)

        print green_ok()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--filter", help="Run specific tests")
    parser.add_argument("--release", help="Use release build of Deno",
                        action="store_true")
    parser.add_argument("--executable", help="Use external executable of Deno")
    args = parser.parse_args()

    target = "release" if args.release else "debug"

    build_dir = None
    if "DENO_BUILD_PATH" in os.environ:
        build_dir = os.environ["DENO_BUILD_PATH"]
    else:
        build_dir = os.path.join(root_path, "target", target)

    deno_dir = os.path.join(build_dir, ".deno_test")
    if os.path.isdir(deno_dir):
        rmtree(deno_dir)
    os.environ["DENO_DIR"] = deno_dir

    deno_exe = os.path.join(build_dir, "deno" + executable_suffix)
    if args.executable:
        deno_exe = args.executable

    http_server.spawn()

    integration_tests(deno_exe, args.filter)


if __name__ == "__main__":
    sys.exit(main())
