# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
import re
import subprocess
import sys

# This script executes libool and filters out logspam lines like:
#    '/path/to/libtool: file: foo.o has no symbols'

BLACKLIST_PATTERNS = map(re.compile, [
    r'^.*libtool: (?:for architecture: \S* )?file: .* has no symbols$',
    r'^.*libtool: warning for library: .* the table of contents is empty '
        r'\(no object file members in the library define global symbols\)$',
    r'^.*libtool: warning same member name \(\S*\) in output file used for '
        r'input files: \S* and: \S* \(due to use of basename, truncation, '
        r'blank padding or duplicate input files\)$',
])


def IsBlacklistedLine(line):
  """Returns whether the line should be filtered out."""
  for pattern in BLACKLIST_PATTERNS:
    if pattern.match(line):
      return True
  return False


def Main(cmd_list):
  env = os.environ.copy()
  # Ref:
  # http://www.opensource.apple.com/source/cctools/cctools-809/misc/libtool.c
  # The problem with this flag is that it resets the file mtime on the file to
  # epoch=0, e.g. 1970-1-1 or 1969-12-31 depending on timezone.
  env['ZERO_AR_DATE'] = '1'
  libtoolout = subprocess.Popen(cmd_list, stderr=subprocess.PIPE, env=env)
  _, err = libtoolout.communicate()
  for line in err.splitlines():
    if not IsBlacklistedLine(line):
      print >>sys.stderr, line
  # Unconditionally touch the output .a file on the command line if present
  # and the command succeeded. A bit hacky.
  if not libtoolout.returncode:
    for i in range(len(cmd_list) - 1):
      if cmd_list[i] == '-o' and cmd_list[i+1].endswith('.a'):
        os.utime(cmd_list[i+1], None)
        break
  return libtoolout.returncode


if __name__ == '__main__':
  sys.exit(Main(sys.argv[1:]))
