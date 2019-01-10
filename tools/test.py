#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# Runs the full test suite.
# Usage: ./tools/test.py out/Debug
import os
import sys
from integration_tests import integration_tests
from deno_dir_test import deno_dir_test
from setup_test import setup_test
from util import build_path, enable_ansi_colors, executable_suffix, run, rmtree
from unit_tests import unit_tests
from util_test import util_test
from benchmark_test import benchmark_test
from repl_test import repl_tests
from prefetch_test import prefetch_test
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

    deno_dir = os.path.join(build_dir, ".deno_test")
    if os.path.isdir(deno_dir):
        rmtree(deno_dir)
    os.environ["DENO_DIR"] = deno_dir

    enable_ansi_colors()

    http_server.spawn()

    deno_exe = os.path.join(build_dir, "deno" + executable_suffix)
    check_exists(deno_exe)

    # Internal tools testing
    setup_test()
    util_test()
    benchmark_test(build_dir, deno_exe)

    test_cc = os.path.join(build_dir, "test_cc" + executable_suffix)
    check_exists(test_cc)
    run([test_cc])

    test_rs = os.path.join(build_dir, "test_rs" + executable_suffix)
    check_exists(test_rs)
    run([test_rs])

    unit_tests(deno_exe)

    prefetch_test(deno_exe)

    integration_tests(deno_exe)

    # TODO We currently skip testing the prompt in Windows completely.
    # Windows does not support the pty module used for testing the permission
    # prompt.
    if os.name != 'nt':
        from permission_prompt_test import permission_prompt_test
        permission_prompt_test(deno_exe)

    repl_tests(deno_exe)

    rmtree(deno_dir)

    deno_dir_test(deno_exe, deno_dir)


if __name__ == '__main__':
    sys.exit(main(sys.argv))
