# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import os
import subprocess
import sys

if __name__ == '__main__':
  parser = argparse.ArgumentParser(
      description='A script to execute a command via xcrun.')
  parser.add_argument('--stamp', action='store', type=str,
      help='Write a stamp file to this path on success.')
  parser.add_argument('--developer_dir', required=False,
                      help='Path to Xcode.')
  args, unknown_args = parser.parse_known_args()

  if args.developer_dir:
    os.environ['DEVELOPER_DIR'] = args.developer_dir

  rv = subprocess.check_call(['xcrun'] + unknown_args)
  if rv == 0 and args.stamp:
    if os.path.exists(args.stamp):
      os.unlink(args.stamp)
    open(args.stamp, 'w+').close()

  sys.exit(rv)
