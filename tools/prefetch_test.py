#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import sys
from util import tests_path, run_output, build_path, executable_suffix, green_ok
import tempfile
import shutil


def prefetch_test(deno_exe):
    sys.stdout.write("prefetch_test...")
    sys.stdout.flush()

    deno_dir = tempfile.mkdtemp()
    try:
        t = os.path.join(tests_path, "006_url_imports.ts")
        output = run_output([deno_exe, "--prefetch", t],
                            env={"DENO_DIR": deno_dir})
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
