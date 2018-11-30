#!/usr/bin/env python
#
# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Invokes Android's aidl
"""

import optparse
import os
import re
import sys
import zipfile

from util import build_utils


def main(argv):
  option_parser = optparse.OptionParser()
  option_parser.add_option('--aidl-path', help='Path to the aidl binary.')
  option_parser.add_option('--imports', help='Files to import.')
  option_parser.add_option('--includes',
                           help='Directories to add as import search paths.')
  option_parser.add_option('--srcjar', help='Path for srcjar output.')
  options, args = option_parser.parse_args(argv[1:])

  with build_utils.TempDir() as temp_dir:
    for f in args:
      classname = os.path.splitext(os.path.basename(f))[0]
      output = os.path.join(temp_dir, classname + '.java')
      aidl_cmd = [options.aidl_path]
      aidl_cmd += [
        '-p' + s for s in build_utils.ParseGnList(options.imports)
      ]
      if options.includes is not None:
        aidl_cmd += [
          '-I' + s for s in build_utils.ParseGnList(options.includes)
        ]
      aidl_cmd += [
        f,
        output
      ]
      build_utils.CheckOutput(aidl_cmd)

    with build_utils.AtomicOutput(options.srcjar) as f:
      with zipfile.ZipFile(f, 'w') as srcjar:
        for path in build_utils.FindInDirectory(temp_dir, '*.java'):
          with open(path) as fileobj:
            data = fileobj.read()
          pkg_name = re.search(r'^\s*package\s+(.*?)\s*;', data, re.M).group(1)
          arcname = '%s/%s' % (
              pkg_name.replace('.', '/'), os.path.basename(path))
          build_utils.AddToZipHermetic(srcjar, arcname, data=data)


if __name__ == '__main__':
  sys.exit(main(sys.argv))
