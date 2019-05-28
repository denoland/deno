#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import sys
from util import mkdtemp, root_path, tests_path, run, green_ok
import shutil
import json


def fmt_test(deno_exe):
    sys.stdout.write("fmt_test...")
    sys.stdout.flush()
    d = mkdtemp()
    try:
        fixed_filename = os.path.join(tests_path, "badly_formatted_fixed.js")
        src = os.path.join(tests_path, "badly_formatted.js")
        dst = os.path.join(d, "badly_formatted.js")
        shutil.copyfile(src, dst)
        # Set DENO_DIR to the temp dir so we test an initial fetch of prettier.
        # TODO(ry) This make the test depend on internet access which is not
        # ideal. We should have prettier in the repo already, and we could
        # fetch it instead through tools/http_server.py.
        deno_dir = d

        # TODO(kt3k) The below line should be run([deno_exe, "fmt", dst], ...)
        # It should be updated when the below issue is addressed
        # https://github.com/denoland/deno_std/issues/330
        run([os.path.join(root_path, deno_exe), "fmt", "badly_formatted.js"],
            cwd=d,
            merge_env={"DENO_DIR": deno_dir})
        with open(fixed_filename) as f:
            expected = f.read()
        with open(dst) as f:
            actual = f.read()
        if expected != actual:
            print "Expected didn't match actual."
            print "expected: ", json.dumps(expected)
            print "actual: ", json.dumps(actual)
            sys.exit(1)

    finally:
        shutil.rmtree(d)
    print green_ok()


if __name__ == "__main__":
    fmt_test(sys.argv[1])
