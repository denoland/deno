#!/usr/bin/env python
from util import run
import sys


# We want to test many ops in deno which have different behavior depending on
# the permissions set. These tests can specify which permissions they expect,
# which appends a special string like "permW1N0" to the end of the test name.
# Here we run several copies of deno with different permissions, filtering the
# tests by the special string. permW0N0 means allow-write but not allow-net.
# See js/test_util.ts for more details.
def unit_tests(deno_exe):
    run([deno_exe, "js/unit_tests.ts", "permW0N0"])
    run([deno_exe, "js/unit_tests.ts", "permW1N0", "--allow-write"])
    run([deno_exe, "js/unit_tests.ts", "permW0N1", "--allow-net"])
    run([
        deno_exe, "js/unit_tests.ts", "permW1N1", "--allow-write",
        "--allow-net"
    ])


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/unit_tests.py out/debug/deno"
        sys.exit(1)
    unit_tests(sys.argv[1])
