#!/usr/bin/env python
# This program fails if ./tools/format.ts changes any files.

import sys
import util
import sys
import subprocess


def main():
    util.run(["tools/format.ts"])
    output = util.run_output(
        ["git", "status", "-uno", "--porcelain", "--ignore-submodules"])
    if len(output) > 0:
        print "Run tools/format.ts "
        print output
        sys.exit(1)


if __name__ == '__main__':
    main()
