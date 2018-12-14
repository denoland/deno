#!/usr/bin/env python
from util import run
import os
import sys
import time


def main(_argv):
    run([sys.executable, "./tools/build.py", "-C", "target/release", "-j2"])
    run([sys.executable, "./tools/test.py"],
        merge_env={"DENO_BUILD_MODE": "release"})
    run(["cargo", "build", "-vv", "-j2", "--release"])


if __name__ == '__main__':
    sys.exit(main(sys.argv))
