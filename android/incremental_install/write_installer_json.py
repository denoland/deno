#!/usr/bin/env python

# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Writes a .json file with the per-apk details for an incremental install."""

import argparse
import json
import os
import sys

sys.path.append(os.path.join(os.path.dirname(__file__), os.pardir, 'gyp'))

from util import build_utils


def _ParseArgs(args):
  args = build_utils.ExpandFileArgs(args)
  parser = argparse.ArgumentParser()
  parser.add_argument('--output-path',
                      help='Output path for .json file.',
                      required=True)
  parser.add_argument('--apk-path',
                      help='Path to .apk relative to output directory.',
                      required=True)
  parser.add_argument('--split',
                      action='append',
                      dest='split_globs',
                      default=[],
                      help='A glob matching the apk splits. '
                           'Can be specified multiple times.')
  parser.add_argument('--native-libs-list',
                      action='append',
                      default=[],
                      help='GN-list of paths to native libraries relative to '
                           'output directory. Can be repeated.')
  parser.add_argument('--dex-file',
                      action='append',
                      default=[],
                      dest='dex_files',
                      help='.dex file to include relative to output directory. '
                           'Can be repeated')
  parser.add_argument('--dex-file-list',
                      help='GN-list of dex paths relative to output directory.')
  parser.add_argument('--show-proguard-warning',
                      action='store_true',
                      default=False,
                      help='Print a warning about proguard being disabled')
  parser.add_argument('--dont-even-try',
                      help='Prints the given message and exits.')

  options = parser.parse_args(args)
  options.dex_files += build_utils.ParseGnList(options.dex_file_list)
  all_libs = []
  for gn_list in options.native_libs_list:
    all_libs.extend(build_utils.ParseGnList(gn_list))
  options.native_libs_list = all_libs
  return options


def main(args):
  options = _ParseArgs(args)

  data = {
      'apk_path': options.apk_path,
      'native_libs': options.native_libs_list,
      'dex_files': options.dex_files,
      'dont_even_try': options.dont_even_try,
      'show_proguard_warning': options.show_proguard_warning,
      'split_globs': options.split_globs,
  }

  with build_utils.AtomicOutput(options.output_path) as f:
    json.dump(data, f, indent=2, sort_keys=True)


if __name__ == '__main__':
  main(sys.argv[1:])
