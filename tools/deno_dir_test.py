#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# Check deno dir is created properly
# Usage: deno_dir_test.py [path to deno dir]
import os
import subprocess
import sys
from util import rmtree, run


def deno_dir_test(deno_exe, deno_dir):
    assert os.path.isfile(deno_exe)

    old_deno_dir = None
    if "DENO_DIR" in os.environ:
        old_deno_dir = os.environ["DENO_DIR"]
        del os.environ["DENO_DIR"]

    if os.path.isdir(deno_dir):
        rmtree(deno_dir)

    # Run deno with no env flag
    run_deno(deno_exe)
    assert not os.path.isdir(deno_dir)

    # Run deno with DENO_DIR env flag
    run_deno(deno_exe, deno_dir)
    assert os.path.isdir(deno_dir)
    assert os.path.isdir(os.path.join(deno_dir, "deps"))
    assert os.path.isdir(os.path.join(deno_dir, "gen"))
    rmtree(deno_dir)

    if old_deno_dir is not None:
        os.environ["DENO_DIR"] = old_deno_dir


def run_deno(deno_exe, deno_dir=None):
    cmd = [deno_exe, "tests/002_hello.ts"]
    deno_dir_env = {"DENO_DIR": deno_dir} if deno_dir is not None else None
    run(cmd, quiet=True, env=deno_dir_env)


USAGE = "./tools/deno_dir_test.py target/debug/deno target/debug/.deno_dir"


def main(argv):
    if len(sys.argv) != 3:
        print "Usage: " + USAGE
        sys.exit(1)
    deno_dir_test(argv[1], argv[2])


if __name__ == '__main__':
    sys.exit(main(sys.argv))
