#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import util
import sys
import subprocess
import itertools


def run_unit_test2(cmd):
    process = subprocess.Popen(
        cmd,
        bufsize=1,
        universal_newlines=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT)
    (actual, expected) = util.parse_unit_test_output(process.stdout, True)
    process.wait()
    errcode = process.returncode
    if errcode != 0:
        sys.exit(errcode)

    if actual == None and expected == None:
        raise AssertionError("Bad js/unit_test.ts output")
    if expected != actual:
        print "expected", expected, "actual", actual
        raise AssertionError("expected tests did not equal actual")

    process.wait()
    errcode = process.returncode
    if errcode != 0:
        sys.exit(errcode)


def run_unit_test(deno_exe, flags=None):
    if flags is None:
        flags = []
    cmd = [deno_exe, "run"] + flags + ["js/unit_tests.ts"]
    print "Running unit tests for permissions: {}".format(flags)
    run_unit_test2(cmd)


perms = [
    "--allow-read", "--allow-write", "--allow-net", "--allow-run",
    "--allow-env", "--allow-high-precision"
]


def unit_tests(deno_exe):
    run_unit_test(deno_exe, [])

    for i in range(len(perms)):
        combinations = itertools.combinations(perms, i + 1)

        for test_perms in combinations:
            test_perms = list(test_perms)
            run_unit_test(deno_exe, test_perms)


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/unit_tests.py target/debug/deno"
        sys.exit(1)
    unit_tests(sys.argv[1])
