#!/usr/bin/env python
#
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import json
import os
import sys
import tempfile
import zipfile

from util import build_utils
from util import proguard_util


def main(args):
  parser = argparse.ArgumentParser()
  build_utils.AddDepfileOption(parser)
  parser.add_argument('--shrinked-android-path', required=True,
                      help='Path to shrinkedAndroid.jar')
  parser.add_argument('--dx-path', required=True,
                      help='Path to dx.jar')
  parser.add_argument('--main-dex-rules-path', action='append', default=[],
                      dest='main_dex_rules_paths',
                      help='A file containing a list of proguard rules to use '
                           'in determining the class to include in the '
                           'main dex.')
  parser.add_argument('--main-dex-list-path', required=True,
                      help='The main dex list file to generate.')
  parser.add_argument('--inputs',
                      help='JARs for which a main dex list should be '
                           'generated.')
  parser.add_argument('--proguard-path', required=True,
                      help='Path to the proguard executable.')
  parser.add_argument('--negative-main-dex-globs',
      help='GN-list of globs of .class names (e.g. org/chromium/foo/Bar.class) '
           'that will fail the build if they match files in the main dex.')

  parser.add_argument('paths', nargs='*', default=[],
                      help='JARs for which a main dex list should be '
                           'generated.')

  args = parser.parse_args(build_utils.ExpandFileArgs(args))

  depfile_deps = []
  if args.inputs:
    args.inputs = build_utils.ParseGnList(args.inputs)
    depfile_deps = args.inputs
    args.paths.extend(args.inputs)

  if args.negative_main_dex_globs:
    args.negative_main_dex_globs = build_utils.ParseGnList(
        args.negative_main_dex_globs)

  proguard_cmd = [
    'java', '-jar', args.proguard_path,
    '-forceprocessing',
    '-dontwarn', '-dontoptimize', '-dontobfuscate', '-dontpreverify',
    '-libraryjars', args.shrinked_android_path,
  ]
  for m in args.main_dex_rules_paths:
    proguard_cmd.extend(['-include', m])

  main_dex_list_cmd = [
    'java', '-cp', args.dx_path,
    'com.android.multidex.MainDexListBuilder',
    # This workaround significantly increases main dex size and doesn't seem to
    # be needed by Chrome. See comment in the source:
    # https://android.googlesource.com/platform/dalvik/+/master/dx/src/com/android/multidex/MainDexListBuilder.java
    '--disable-annotation-resolution-workaround',
  ]

  input_paths = list(args.paths)
  input_paths += [
    args.shrinked_android_path,
    args.dx_path,
  ]
  input_paths += args.main_dex_rules_paths

  input_strings = [
    proguard_cmd,
    main_dex_list_cmd,
  ]
  if args.negative_main_dex_globs:
    input_strings += args.negative_main_dex_globs

  output_paths = [
    args.main_dex_list_path,
  ]

  build_utils.CallAndWriteDepfileIfStale(
      lambda: _OnStaleMd5(proguard_cmd, main_dex_list_cmd, args.paths,
                          args.main_dex_list_path,
                          args.negative_main_dex_globs),
      args,
      input_paths=input_paths,
      input_strings=input_strings,
      output_paths=output_paths,
      depfile_deps=depfile_deps,
      add_pydeps=False)

  return 0


def _CheckForUnwanted(kept_classes, proguard_cmd, negative_main_dex_globs):
  # Check if ProGuard kept any unwanted classes.
  found_unwanted_classes = sorted(
      p for p in kept_classes
      if build_utils.MatchesGlob(p, negative_main_dex_globs))

  if found_unwanted_classes:
    first_class = found_unwanted_classes[0].replace(
        '.class', '').replace('/', '.')
    proguard_cmd += ['-whyareyoukeeping', 'class', first_class, '{}']
    output = build_utils.CheckOutput(
        proguard_cmd, print_stderr=False,
        stdout_filter=proguard_util.ProguardOutputFilter())
    raise Exception(
        ('Found classes that should not be in the main dex:\n    {}\n\n'
         'Here is the -whyareyoukeeping output for {}: \n{}').format(
             '\n    '.join(found_unwanted_classes), first_class, output))


def _OnStaleMd5(proguard_cmd, main_dex_list_cmd, paths, main_dex_list_path,
                negative_main_dex_globs):
  paths_arg = ':'.join(paths)
  main_dex_list = ''
  try:
    with tempfile.NamedTemporaryFile(suffix='.jar') as temp_jar:
      # Step 1: Use ProGuard to find all @MainDex code, and all code reachable
      # from @MainDex code (recursive).
      proguard_cmd += [
        '-injars', paths_arg,
        '-outjars', temp_jar.name
      ]
      build_utils.CheckOutput(proguard_cmd, print_stderr=False)

      # Record the classes kept by ProGuard. Not used by the build, but useful
      # for debugging what classes are kept by ProGuard vs. MainDexListBuilder.
      with zipfile.ZipFile(temp_jar.name) as z:
        kept_classes = [p for p in z.namelist() if p.endswith('.class')]
      with open(main_dex_list_path + '.partial', 'w') as f:
        f.write('\n'.join(kept_classes) + '\n')

      if negative_main_dex_globs:
        # Perform assertions before MainDexListBuilder because:
        # a) MainDexListBuilder is not recursive, so being included by it isn't
        #    a huge deal.
        # b) Errors are much more actionable.
        _CheckForUnwanted(kept_classes, proguard_cmd, negative_main_dex_globs)

      # Step 2: Expand inclusion list to all classes referenced by the .class
      # files of kept classes (non-recursive).
      main_dex_list_cmd += [
        temp_jar.name, paths_arg
      ]
      main_dex_list = build_utils.CheckOutput(main_dex_list_cmd)

  except build_utils.CalledProcessError as e:
    if 'output jar is empty' in e.output:
      pass
    elif "input doesn't contain any classes" in e.output:
      pass
    else:
      raise

  with open(main_dex_list_path, 'w') as main_dex_list_file:
    main_dex_list_file.write(main_dex_list)


if __name__ == '__main__':
  sys.exit(main(sys.argv[1:]))
