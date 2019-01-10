#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import sys
from util import run_output, build_path, executable_suffix, green_ok
import tempfile
import shutil


def prefetch_test(deno_exe):
    deno_dir = tempfile.mkdtemp()
    try:
        output = run_output([
            deno_exe, "--prefetch",
            "http://127.0.0.1:4545/tests/005_more_imports.ts"
        ],
                            env={"DENO_DIR": deno_dir})
        assert output == ""

        # Check that we actually prefetched something.
        os.path.exists(
            os.path.join(
                deno_dir,
                "deps/http/127.0.0.1_PORT4545/tests/005_more_imports.ts"))

        print "prefetch_test...", green_ok()

    finally:
        shutil.rmtree(deno_dir)


if __name__ == "__main__":
    prefetch_test(sys.argv[1])
