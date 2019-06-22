#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import shutil

from test_util import DenoTestCase, run_tests
from util import mkdtemp, root_path, tests_path, run_output


class TestFmt(DenoTestCase):
    def test_fmt(self):
        d = mkdtemp()
        try:
            fixed_filename = os.path.join(tests_path,
                                          "badly_formatted_fixed.js")
            src = os.path.join(tests_path, "badly_formatted.js")
            dst = os.path.join(d, "badly_formatted.js")
            shutil.copyfile(src, dst)

            # Set DENO_DIR to the temp dir to test an initial fetch of prettier.
            # TODO(ry) This make the test depend on internet access which is not
            # ideal. We should have prettier in the repo already, and we could
            # fetch it instead through tools/http_server.py.
            deno_dir = d

            result = run_output(
                [os.path.join(root_path, self.deno_exe), "fmt", dst],
                cwd=d,
                merge_env={"DENO_DIR": deno_dir},
                exit_on_fail=True,
                quiet=True)
            self.assertEqual(result.code, 0)
            with open(fixed_filename) as f:
                expected = f.read()
            with open(dst) as f:
                actual = f.read()
            self.assertEqual(expected, actual)
        finally:
            shutil.rmtree(d)


if __name__ == "__main__":
    run_tests()
