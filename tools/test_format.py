#!/usr/bin/env python
# This program fails if ./tools/format.py changes any files.

import sys
import util
import sys
import subprocess


def main():
    util.run([sys.executable, "tools/format.py"])
    output = util.run_output(
        ["git", "status", "-uno", "--porcelain", "--ignore-submodules"])
    if len(output) > 0:
        print "Run tools/format.py "
        print output
        sys.exit(1)


if __name__ == '__main__':
    main()
