#!/usr/bin/env python
#
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Clobbers a CIPD root."""

import argparse
import os
import shutil
import sys


def main():
  parser = argparse.ArgumentParser(
      description='Clobbers the CIPD root in the given directory.')

  parser.add_argument(
      '--root',
      required=True,
      help='Root directory for dependency.')
  args = parser.parse_args()

  cipd_root_dir = os.path.join(args.root, '.cipd')
  if os.path.exists(cipd_root_dir):
    shutil.rmtree(cipd_root_dir)

  return 0


if __name__ == '__main__':
  sys.exit(main())
