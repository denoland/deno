#!/usr/bin/env python
#
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Creates an Android .aar file."""

import argparse
import os
import posixpath
import shutil
import sys
import tempfile
import zipfile

from util import build_utils


_ANDROID_BUILD_DIR = os.path.dirname(os.path.dirname(__file__))


def _MergeRTxt(r_paths):
  """Merging the given R.txt files and returns them as a string."""
  all_lines = set()
  for r_path in r_paths:
    with open(r_path) as f:
      all_lines.update(f.readlines())
  return ''.join(sorted(all_lines))


def _MergeProguardConfigs(proguard_configs):
  """Merging the given proguard config files and returns them as a string."""
  ret = []
  for config in proguard_configs:
    ret.append('# FROM: {}'.format(config))
    with open(config) as f:
      ret.append(f.read())
  return '\n'.join(ret)


def _AddResources(aar_zip, resource_zips):
  """Adds all resource zips to the given aar_zip.

  Ensures all res/values/* files have unique names by prefixing them.
  """
  for i, path in enumerate(resource_zips):
    with zipfile.ZipFile(path) as res_zip:
      for info in res_zip.infolist():
        data = res_zip.read(info)
        dirname, basename = posixpath.split(info.filename)
        if 'values' in dirname:
          basename = '{}_{}'.format(basename, i)
          info.filename = posixpath.join(dirname, basename)
        info.filename = posixpath.join('res', info.filename)
        aar_zip.writestr(info, data)


def main(args):
  args = build_utils.ExpandFileArgs(args)
  parser = argparse.ArgumentParser()
  build_utils.AddDepfileOption(parser)
  parser.add_argument('--output', required=True, help='Path to output aar.')
  parser.add_argument('--jars', required=True, help='GN list of jar inputs.')
  parser.add_argument('--dependencies-res-zips', required=True,
                      help='GN list of resource zips')
  parser.add_argument('--r-text-files', required=True,
                      help='GN list of R.txt files to merge')
  parser.add_argument('--proguard-configs', required=True,
                      help='GN list of ProGuard flag files to merge.')
  parser.add_argument(
      '--android-manifest',
      help='Path to AndroidManifest.xml to include.',
      default=os.path.join(_ANDROID_BUILD_DIR, 'AndroidManifest.xml'))
  parser.add_argument('--native-libraries', default='',
                      help='GN list of native libraries. If non-empty then '
                      'ABI must be specified.')
  parser.add_argument('--abi',
                      help='ABI (e.g. armeabi-v7a) for native libraries.')

  options = parser.parse_args(args)

  if options.native_libraries and not options.abi:
    parser.error('You must provide --abi if you have native libs')

  options.jars = build_utils.ParseGnList(options.jars)
  options.dependencies_res_zips = build_utils.ParseGnList(
      options.dependencies_res_zips)
  options.r_text_files = build_utils.ParseGnList(options.r_text_files)
  options.proguard_configs = build_utils.ParseGnList(options.proguard_configs)
  options.native_libraries = build_utils.ParseGnList(options.native_libraries)

  with tempfile.NamedTemporaryFile(delete=False) as staging_file:
    try:
      with zipfile.ZipFile(staging_file.name, 'w') as z:
        build_utils.AddToZipHermetic(
            z, 'AndroidManifest.xml', src_path=options.android_manifest)

        with tempfile.NamedTemporaryFile() as jar_file:
          build_utils.MergeZips(jar_file.name, options.jars)
          build_utils.AddToZipHermetic(z, 'classes.jar', src_path=jar_file.name)

        build_utils.AddToZipHermetic(
            z, 'R.txt', data=_MergeRTxt(options.r_text_files))
        build_utils.AddToZipHermetic(z, 'public.txt', data='')

        if options.proguard_configs:
          build_utils.AddToZipHermetic(
              z, 'proguard.txt',
              data=_MergeProguardConfigs(options.proguard_configs))

        _AddResources(z, options.dependencies_res_zips)

        for native_library in options.native_libraries:
          libname = os.path.basename(native_library)
          build_utils.AddToZipHermetic(
              z, os.path.join('jni', options.abi, libname),
              src_path=native_library)
    except:
      os.unlink(staging_file.name)
      raise
    shutil.move(staging_file.name, options.output)

  if options.depfile:
    all_inputs = (options.jars + options.dependencies_res_zips +
                  options.r_text_files + options.proguard_configs)
    build_utils.WriteDepfile(options.depfile, options.output, all_inputs,
                             add_pydeps=False)


if __name__ == '__main__':
  main(sys.argv[1:])
