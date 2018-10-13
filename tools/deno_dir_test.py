#!/usr/bin/env python
# Check deno dir is created properly
# Usage: deno_dir_test.py [path to deno dir]
import os
import sys

def deno_dir_test(deno_dir):
    assert os.path.isdir(deno_dir)
    assert os.path.isdir(os.path.join(deno_dir, "deps"))
    assert os.path.isdir(os.path.join(deno_dir, "gen"))


def main(argv):
    deno_dir_test(argv[1])


if __name__ == '__main__':
    sys.exit(main(sys.argv))
