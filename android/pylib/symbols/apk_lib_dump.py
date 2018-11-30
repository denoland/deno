#!/usr/bin/env python

# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Dump shared library information from an APK file.

This script is used to dump which *uncompressed* native shared libraries an
APK contains, as well as their position within the file. This is mostly useful
to diagnose logcat and tombstone symbolization issues when the libraries are
loaded directly from the APK at runtime.

The default format will print one line per uncompressed shared library with the
following format:

  0x<start-offset> 0x<end-offset> 0x<file-size> <file-path>

The --format=python option can be used to dump the same information that is
easy to use in a Python script, e.g. with a line like:

  (0x<start-offset>, 0x<end-offset>, 0x<file-size>, <file-path>),
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..'))

from pylib.symbols import apk_native_libs

def main():
  parser = argparse.ArgumentParser(
      description=__doc__,
      formatter_class=argparse.RawDescriptionHelpFormatter)

  parser.add_argument('apk', help='Input APK file path.')

  parser.add_argument('--format', help='Select output format',
                      default='default', choices=['default', 'python'])

  args = parser.parse_args()

  apk_reader = apk_native_libs.ApkReader(args.apk)
  lib_map = apk_native_libs.ApkNativeLibraries(apk_reader)
  for lib_path, file_offset, file_size in lib_map.GetDumpList():
    if args.format == 'python':
      print '(0x%08x, 0x%08x, 0x%08x, \'%s\'),' % (
          file_offset, file_offset + file_size, file_size, lib_path)
    else:
      print '0x%08x 0x%08x 0x%08x %s' % (
          file_offset, file_offset + file_size, file_size, lib_path)

  return 0


if __name__ == '__main__':
  sys.exit(main())
