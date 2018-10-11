#!/usr/bin/env python
import util
import sys
import subprocess
import re


def run_unit_test(deno_exe, permStr, flags=[]):
    cmd = [deno_exe, "--reload", "js/unit_tests.ts", permStr] + flags
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


# We want to test many ops in deno which have different behavior depending on
# the permissions set. These tests can specify which permissions they expect,
# which appends a special string like "permW1N0" to the end of the test name.
# Here we run several copies of deno with different permissions, filtering the
# tests by the special string. permW0N0 means allow-write but not allow-net.
# See js/test_util.ts for more details.
def unit_tests(deno_exe):
    run_unit_test(deno_exe, "permW0N0E0")
    run_unit_test(deno_exe, "permW1N0E0", ["--allow-write"])
    run_unit_test(deno_exe, "permW0N1E0", ["--allow-net"])
    run_unit_test(deno_exe, "permW0N0E1", ["--allow-env"])
    # TODO We might accidentally miss some. We should be smarter about which we
    # run. Maybe we can use the "filtered out" number to check this.


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/unit_tests.py out/debug/deno"
        sys.exit(1)
    unit_tests(sys.argv[1])
