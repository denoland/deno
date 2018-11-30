#!/usr/bin/env python

# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Aggregates EMMA coverage files to produce html output."""

import fnmatch
import json
import optparse
import os
import sys

import devil_chromium
from devil.utils import cmd_helper
from pylib import constants
from pylib.constants import host_paths


def _GetFilesWithExt(root_dir, ext):
  """Gets all files with a given extension.

  Args:
    root_dir: Directory in which to search for files.
    ext: Extension to look for (including dot)

  Returns:
    A list of absolute paths to files that match.
  """
  files = []
  for root, _, filenames in os.walk(root_dir):
    basenames = fnmatch.filter(filenames, '*.' + ext)
    files.extend([os.path.join(root, basename)
                  for basename in basenames])

  return files


def main():
  option_parser = optparse.OptionParser()
  option_parser.add_option('--output', help='HTML output filename.')
  option_parser.add_option('--coverage-dir', default=None,
                           help=('Root of the directory in which to search for '
                                 'coverage data (.ec) files.'))
  option_parser.add_option('--metadata-dir', default=None,
                           help=('Root of the directory in which to search for '
                                 'coverage metadata (.em) files.'))
  option_parser.add_option('--cleanup', action='store_true',
                           help=('If set, removes coverage files generated at '
                                 'runtime.'))
  options, _ = option_parser.parse_args()

  devil_chromium.Initialize()

  if not (options.coverage_dir and options.metadata_dir and options.output):
    option_parser.error('One or more mandatory options are missing.')

  coverage_files = _GetFilesWithExt(options.coverage_dir, 'ec')
  metadata_files = _GetFilesWithExt(options.metadata_dir, 'em')
  # Filter out zero-length files. These are created by emma_instr.py when a
  # target has no classes matching the coverage filter.
  metadata_files = [f for f in metadata_files if os.path.getsize(f)]
  print 'Found coverage files: %s' % str(coverage_files)
  print 'Found metadata files: %s' % str(metadata_files)

  sources = []
  for f in metadata_files:
    sources_file = os.path.splitext(f)[0] + '_sources.txt'
    with open(sources_file, 'r') as sf:
      sources.extend(json.load(sf))

  # Source paths should be passed to EMMA in a way that the relative file paths
  # reflect the class package name.
  PARTIAL_PACKAGE_NAMES = ['com/google', 'org/chromium', 'com/chrome']
  fixed_source_paths = set()

  for path in sources:
    for partial in PARTIAL_PACKAGE_NAMES:
      if partial in path:
        fixed_path = os.path.join(
            host_paths.DIR_SOURCE_ROOT, path[:path.index(partial)])
        fixed_source_paths.add(fixed_path)
        break

  sources = list(fixed_source_paths)

  input_args = []
  for f in coverage_files + metadata_files:
    input_args.append('-in')
    input_args.append(f)

  output_args = ['-Dreport.html.out.file', options.output,
                 '-Dreport.html.out.encoding', 'UTF-8']
  source_args = ['-sp', ','.join(sources)]

  exit_code = cmd_helper.RunCmd(
      ['java', '-cp',
       os.path.join(constants.ANDROID_SDK_ROOT, 'tools', 'lib', 'emma.jar'),
       'emma', 'report', '-r', 'html']
      + input_args + output_args + source_args)

  if options.cleanup:
    for f in coverage_files:
      os.remove(f)

  # Command tends to exit with status 0 when it actually failed.
  if not exit_code and not os.path.exists(options.output):
    exit_code = 1

  return exit_code


if __name__ == '__main__':
  sys.exit(main())
