#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Checks the number of static initializers in an APK's library."""

import argparse
import os
import sys

sys.path.append(os.path.join(os.path.dirname(__file__), '..'))
import resource_sizes

from util import build_utils


def main():
  parser = argparse.ArgumentParser()
  build_utils.AddDepfileOption(parser)
  parser.add_argument('--touch', help='File to touch upon success')
  parser.add_argument('--tool-prefix', required=True,
                      help='Prefix for nm and friends')
  parser.add_argument('--expected-count', required=True, type=int,
                      help='Fail if number of static initializers is not '
                           'equal to this value.')
  parser.add_argument('apk', help='APK file path.')
  args = parser.parse_args()

  #TODO(crbug.com/838414): add support for files included via loadable_modules.
  ignored_libs = ['libarcore_sdk_c_minimal.so']

  si_count = resource_sizes.AnalyzeStaticInitializers(
      args.apk, args.tool_prefix, False, '.', ignored_libs)
  if si_count != args.expected_count:
    print 'Expected {} static initializers, but found {}.'.format(
        args.expected_count, si_count)
    if args.expected_count > si_count:
      print 'You have removed one or more static initializers. Thanks!'
      print 'To fix the build, update the expectation in:'
      print '    //chrome/android/static_initializers.gni'
    else:
      print 'Dumping static initializers via dump-static-initializers.py:'
      sys.stdout.flush()
      resource_sizes.AnalyzeStaticInitializers(
          args.apk, args.tool_prefix, True, '.', ignored_libs)
      print
      print 'If the above list is not useful, consider listing them with:'
      print '    //tools/binary_size/diagnose_bloat.py'
      print
      print 'For more information:'
      print ('    https://chromium.googlesource.com/chromium/src/+/master/docs/'
             'static_initializers.md')
    sys.exit(1)

  if args.depfile:
    build_utils.WriteDepfile(args.depfile, args.touch)
  if args.touch:
    open(args.touch, 'w')


if __name__ == '__main__':
  main()
