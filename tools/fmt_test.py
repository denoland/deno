#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import json
import os
import shutil
import sys

from util import (DenoTestCase, mkdtemp, root_path, tests_path, run, test_main)


class FmtTest(DenoTestCase):
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

            # TODO(kt3k) Below can be run([deno_exe, "fmt", dst], ...)
            # once the following issue is addressed:
            # https://github.com/denoland/deno_std/issues/330
            run([
                os.path.join(root_path, self.deno_exe), "fmt",
                "badly_formatted.js"
            ],
                cwd=d,
                merge_env={"DENO_DIR": deno_dir})
            with open(fixed_filename) as f:
                expected = f.read()
            with open(dst) as f:
                actual = f.read()
            self.assertEqual(expected, actual)
        finally:
            shutil.rmtree(d)


if __name__ == "__main__":
    test_main()
