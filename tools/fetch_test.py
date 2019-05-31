#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import shutil

import http_server
from test_util import DenoTestCase, run_tests
from util import mkdtemp, tests_path, run_output


class FetchTest(DenoTestCase):
    def test_fetch(self):
        deno_dir = mkdtemp()
        try:
            t = os.path.join(tests_path, "006_url_imports.ts")
            output = run_output([self.deno_exe, "fetch", t],
                                merge_env={"DENO_DIR": deno_dir})
            assert output == ""
            # Check that we actually did the prefetch.
            os.path.exists(
                os.path.join(
                    deno_dir,
                    "deps/http/localhost_PORT4545/tests/subdir/mod2.ts"))
        finally:
            shutil.rmtree(deno_dir)


if __name__ == "__main__":
    with http_server.spawn():
        run_tests()
