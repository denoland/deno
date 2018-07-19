#!/usr/bin/env python
# Runs the full test suite.
# Usage: ./tools/test.py out/Debug
import os
import sys
from check_output_test import check_output_test
from util import run


def main(argv):
    build_dir = argv[1]
    os.path.isdir(build_dir)
    run([os.path.join(build_dir, "test_cc")])
    run([os.path.join(build_dir, "handlers_test")])
    check_output_test(os.path.join(build_dir, "deno"))
    check_output_test(os.path.join(build_dir, "deno_ns"))


if __name__ == '__main__':
    sys.exit(main(sys.argv))
