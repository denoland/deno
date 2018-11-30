#!/usr/bin/env python
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""
Replaces GN files in tree with files from here that
make the build use system libraries.
"""

from __future__ import print_function

import argparse
import os
import shutil
import sys


REPLACEMENTS = {
  'ffmpeg': 'third_party/ffmpeg/BUILD.gn',
  'flac': 'third_party/flac/BUILD.gn',
  'fontconfig': 'third_party/fontconfig/BUILD.gn',
  'freetype': 'build/config/freetype/freetype.gni',
  'harfbuzz-ng': 'third_party/harfbuzz-ng/harfbuzz.gni',
  'icu': 'third_party/icu/BUILD.gn',
  'libdrm': 'third_party/libdrm/BUILD.gn',
  'libevent': 'base/third_party/libevent/BUILD.gn',
  'libjpeg': 'third_party/libjpeg.gni',
  'libpng': 'third_party/libpng/BUILD.gn',
  'libvpx': 'third_party/libvpx/BUILD.gn',
  'libwebp': 'third_party/libwebp/BUILD.gn',
  'libxml': 'third_party/libxml/BUILD.gn',
  'libxslt': 'third_party/libxslt/BUILD.gn',
  'openh264': 'third_party/openh264/BUILD.gn',
  'opus': 'third_party/opus/BUILD.gn',
  're2': 'third_party/re2/BUILD.gn',
  'snappy': 'third_party/snappy/BUILD.gn',
  'yasm': 'third_party/yasm/yasm_assemble.gni',
  'zlib': 'third_party/zlib/BUILD.gn',
}


def DoMain(argv):
  my_dirname = os.path.dirname(__file__)
  source_tree_root = os.path.abspath(
    os.path.join(my_dirname, '..', '..', '..'))

  parser = argparse.ArgumentParser()
  parser.add_argument('--system-libraries', nargs='*', default=[])
  parser.add_argument('--undo', action='store_true')

  args = parser.parse_args(argv)

  handled_libraries = set()
  for lib, path in REPLACEMENTS.items():
    if lib not in args.system_libraries:
      continue
    handled_libraries.add(lib)

    if args.undo:
      # Restore original file, and also remove the backup.
      # This is meant to restore the source tree to its original state.
      os.rename(os.path.join(source_tree_root, path + '.orig'),
                os.path.join(source_tree_root, path))
    else:
      # Create a backup copy for --undo.
      shutil.copyfile(os.path.join(source_tree_root, path),
                      os.path.join(source_tree_root, path + '.orig'))

      # Copy the GN file from directory of this script to target path.
      shutil.copyfile(os.path.join(my_dirname, '%s.gn' % lib),
                      os.path.join(source_tree_root, path))

  unhandled_libraries = set(args.system_libraries) - handled_libraries
  if unhandled_libraries:
    print('Unrecognized system libraries requested: %s' % ', '.join(
        sorted(unhandled_libraries)), file=sys.stderr)
    return 1

  return 0


if __name__ == '__main__':
  sys.exit(DoMain(sys.argv[1:]))
