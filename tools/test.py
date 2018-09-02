#!/usr/bin/env python
# Runs the full test suite.
# Usage: ./tools/test.py out/Debug
import os
import sys
from check_output_test import check_output_test
from setup_test import setup_test
from util import build_path, enable_ansi_colors, executable_suffix, run
from unit_tests import unit_tests
from util_test import util_test
import subprocess
import http_server


def check_exists(filename):
    if not os.path.exists(filename):
        print "Required target doesn't exist:", filename
        print "Run ./tools/build.py"
        sys.exit(1)


def main(argv):
    if len(argv) == 2:
        build_dir = sys.argv[1]
    elif len(argv) == 1:
        build_dir = build_path()
    else:
        print "Usage: tools/test.py [build_dir]"
        sys.exit(1)

    enable_ansi_colors()

    http_server.spawn()

    # Internal tools testing
    setup_test()
    util_test()

    test_cc = os.path.join(build_dir, "test_cc" + executable_suffix)
    check_exists(test_cc)
    run([test_cc])

    test_rs = os.path.join(build_dir, "test_rs" + executable_suffix)
    check_exists(test_rs)
    run([test_rs])

    deno_exe = os.path.join(build_dir, "deno" + executable_suffix)
    check_exists(deno_exe)
    unit_tests(deno_exe)

    check_exists(deno_exe)
    check_output_test(deno_exe)

    deno_ns_exe = os.path.join(build_dir, "deno_ns" + executable_suffix)
    check_exists(deno_ns_exe)
    check_output_test(deno_ns_exe)


if __name__ == '__main__':
    sys.exit(main(sys.argv))
