#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import sys
import subprocess

import http_server
from test_util import DenoTestCase, run_tests


class JsUnitTests(DenoTestCase):
    def test_unit_test_runner(self):
        cmd = [
            self.deno_exe, "run", "--reload", "--allow-run",
            "js/unit_test_runner.ts"
        ]
        process = subprocess.Popen(
            cmd, bufsize=1, universal_newlines=True, stderr=subprocess.STDOUT)

        process.wait()
        errcode = process.returncode
        if errcode != 0:
            raise AssertionError(
                "js/unit_test_runner.ts exited with exit code %s" % errcode)


if __name__ == '__main__':
    with http_server.spawn():
        run_tests()
