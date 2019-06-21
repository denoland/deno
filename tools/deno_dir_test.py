#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Check deno dir is created properly
# Usage: deno_dir_test.py [path to deno dir]
import os

from test_util import DenoTestCase, run_tests
from util import mkdtemp, rmtree, run_output


class TestDenoDir(DenoTestCase):
    def setUp(self):
        self.old_deno_dir = None
        if "DENO_DIR" in os.environ:
            self.old_deno_dir = os.environ["DENO_DIR"]
            del os.environ["DENO_DIR"]

    def tearDown(self):
        if self.old_deno_dir is not None:
            os.environ["DENO_DIR"] = self.old_deno_dir

    def test_deno_dir(self):
        deno_dir = mkdtemp()
        if os.path.isdir(deno_dir):
            rmtree(deno_dir)

        # Run deno with no env flag
        self.run_deno()
        assert not os.path.isdir(deno_dir)

        # Run deno with DENO_DIR env flag
        self.run_deno(deno_dir)
        assert os.path.isdir(deno_dir)
        assert os.path.isdir(os.path.join(deno_dir, "deps"))
        assert os.path.isdir(os.path.join(deno_dir, "gen"))
        rmtree(deno_dir)

    def run_deno(self, deno_dir=None):
        cmd = [self.deno_exe, "run", "tests/002_hello.ts"]
        deno_dir_env = {"DENO_DIR": deno_dir} if deno_dir is not None else None
        res = run_output(cmd, quiet=True, env=deno_dir_env)
        self.assertEqual(res.code, 0)


if __name__ == '__main__':
    run_tests()
