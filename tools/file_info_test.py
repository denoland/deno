#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import sys
from util import tests_path, run, run_output, build_path, green_ok
import tempfile
import shutil
import re


def file_info_test(deno_exe):
    sys.stdout.write("file_info_test...")
    sys.stdout.flush()

    # On Windows, set the base directory that mkdtemp() uses explicitly. If not,
    # it'll use the short (8.3) path to the temp dir, which triggers the error
    # 'TS5009: Cannot find the common subdirectory path for the input files.'
    temp_dir = os.environ["TEMP"] if os.name == 'nt' else None
    deno_dir = tempfile.mkdtemp(dir=temp_dir)
    try:
        t = "https://deno.land/thumb.ts"
        run([deno_exe, t],
            merge_env={"DENO_DIR": deno_dir})
        output = run_output([deno_exe, "--info", t],
                            merge_env={"DENO_DIR": deno_dir})
        cache_location = os.path.join(deno_dir,
                         "deps/https/deno.land/thumb.ts")
        assert cache_location in output
        assert "TypeScript" in output
        # actual source map name changes with path
        compiled_code = re.search(
            'compiled javascript:([^\n]*)\n', output).group(1)
        assert compiled_code + ".map" in output
    finally:
        shutil.rmtree(deno_dir)

    print green_ok()


if __name__ == "__main__":
    file_info_test(sys.argv[1])
