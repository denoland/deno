#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import util
import sys
import subprocess
import http_server


def run_unit_test(cmd):
    process = subprocess.Popen(
        cmd, bufsize=1, universal_newlines=True, stderr=subprocess.STDOUT)

    process.wait()
    errcode = process.returncode
    if errcode != 0:
        sys.exit(errcode)


def unit_tests(deno_exe):
    run_unit_test(
        [deno_exe, "run", "--reload", "--allow-run", "js/unit_test_runner.ts"])


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/unit_tests.py target/debug/deno"
        sys.exit(1)

    http_server.spawn()
    unit_tests(sys.argv[1])
