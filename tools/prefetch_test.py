#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import sys
from util import mkdtemp, tests_path, run_output, green_ok
import shutil


def prefetch_test(deno_exe):
    sys.stdout.write("prefetch_test...")
    sys.stdout.flush()

    deno_dir = mkdtemp()
    try:
        t = os.path.join(tests_path, "006_url_imports.ts")
        output = run_output([deno_exe, "--prefetch", t],
                            merge_env={"DENO_DIR": deno_dir})
        assert output == ""
        # Check that we actually did the prefetch.
        os.path.exists(
            os.path.join(deno_dir,
                         "deps/http/localhost_PORT4545/tests/subdir/mod2.ts"))
    finally:
        shutil.rmtree(deno_dir)

    print green_ok()


if __name__ == "__main__":
    prefetch_test(sys.argv[1])
