# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import subprocess
import sys

# This script executes a command and redirects the stdout to a file. This is
# equivalent to |command... > output_file|.
#
# Usage: python redirect_stdout.py output_file command...

if __name__ == '__main__':
  if len(sys.argv) < 2:
    print >> sys.stderr, "Usage: %s output_file command..." % (sys.argv[0])
    sys.exit(1)

  with open(sys.argv[1], 'w') as fp:
    sys.exit(subprocess.check_call(sys.argv[2:], stdout=fp))
