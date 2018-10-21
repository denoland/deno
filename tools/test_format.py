#!/usr/bin/env python
# This program fails if ./tools/format.py changes any files.

import sys
import util
import sys
import subprocess


def main(argv):
    util.run(["python", "tools/format.py"])
    output = subprocess.check_output(["git", "diff"])
    print "git diff output len", len(output)
    if len(output) > 0:
        sys.exit(1)


if __name__ == '__main__':
    main(sys.argv)
