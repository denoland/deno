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
        # Set DENO_DIR to //js/ so we don't have to rely on an intenet
        # connection to download https://deno.land/std/prettier/main.ts
        deno_dir = os.path.join(root_path, "js")
        run([deno_exe, "fmt", dst], merge_env={"DENO_DIR": deno_dir})
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
