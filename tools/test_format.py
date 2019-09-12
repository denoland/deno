#!/usr/bin/env python
# This program fails if ./tools/format.py changes any files.

import sys
import subprocess
import util


def main():
    util.run([sys.executable, "tools/format.py"])
    result = util.run_output(
        ["git", "status", "-uno", "--porcelain", "--ignore-submodules"],
        exit_on_fail=True)
    if result.out:
        print "Run tools/format.py "
        print result.out
        sys.exit(1)


if __name__ == '__main__':
    main()
