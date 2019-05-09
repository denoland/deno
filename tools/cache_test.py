#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import sys
from util import mkdtemp, tests_path, run_output, green_ok
import shutil


def cache_purge_stale_test(deno_exe):
    sys.stdout.write("fetch_test...")
    sys.stdout.flush()

    deno_dir = mkdtemp()
    gen_dir = os.path.join(deno_dir, "gen")
    filename = os.path.join(deno_dir, "test.ts")

    content_v1 = "console.log('HELLO');"
    content_v2 = "console.log('HELLO WORLD');"

    # Prepare version 1
    with open(filename, "w+") as f:
        f.write(content_v1)

    try:
        # Run version 1, generate compiled files
        run_output([deno_exe, "run", filename],
                   merge_env={"DENO_DIR": deno_dir})

        # Read from gen folder
        files_v1 = [os.path.join(gen_dir, f) for f in os.listdir(gen_dir)]
        files_v1.sort()
        assert len(files_v1) == 3
        gen_js = filter(lambda f: f.endswith(".js"), files_v1)[0]
        with open(gen_js, "r+") as f:
            data = f.read()
            assert "HELLO" in data
            assert not "HELLO WORLD" in data

        # Prepare version 2
        with open(filename, "w+") as f:
            f.write(content_v2)

        # Run version 2, update compiled files
        run_output([deno_exe, "run", filename],
                   merge_env={"DENO_DIR": deno_dir})

        # Read from gen folder again
        files_v2 = [os.path.join(gen_dir, f) for f in os.listdir(gen_dir)]
        files_v2.sort()
        assert len(files_v2) == 3
        # Files should still have the same names
        assert files_v1 == files_v2
        # But the compiled content is updated!
        with open(gen_js, "r+") as f:
            data = f.read()
            assert "HELLO WORLD" in data

        print green_ok()
    finally:
        shutil.rmtree(deno_dir)


def cache_test(deno_exe):
    cache_purge_stale_test(deno_exe)


if __name__ == "__main__":
    cache_test(sys.argv[1])
