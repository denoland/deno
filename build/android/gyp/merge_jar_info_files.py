#!/usr/bin/env python

# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Merges .jar.info files into one for APKs."""

import argparse
import os
import shutil
import sys
import tempfile
import zipfile

from util import build_utils
from util import jar_info_utils


def _FullJavaNameFromClassFilePath(path):
  # Input:  base/android/java/src/org/chromium/Foo.class
  # Output: base.android.java.src.org.chromium.Foo
  if not path.endswith('.class'):
    return ''
  path = os.path.splitext(path)[0]
  parts = []
  while path:
    # Use split to be platform independent.
    head, tail = os.path.split(path)
    path = head
    parts.append(tail)
  parts.reverse()  # Package comes first
  return '.'.join(parts)


def _MergeInfoFiles(output, jar_paths):
  """Merge several .jar.info files to generate an .apk.jar.info.

  Args:
    output: output file path.
    jar_paths: List of .jar file paths for the target apk.
  """
  info_data = dict()
  for jar_path in jar_paths:
    # android_java_prebuilt adds jar files in the src directory (relative to
    #     the output directory, usually ../../third_party/example.jar).
    # android_aar_prebuilt collects jar files in the aar file and uses the
    #     java_prebuilt rule to generate gen/example/classes.jar files.
    # We scan these prebuilt jars to parse each class path for the FQN. This
    #     allows us to later map these classes back to their respective src
    #     directories.
    jar_info_path = jar_path + '.info'
    # TODO(agrieve): This should probably also check that the mtime of the .info
    #     is newer than that of the .jar, or change prebuilts to always output
    #     .info files so that they always exist (and change the depfile to
    #     depend directly on them).
    if os.path.exists(jar_info_path):
      info_data.update(jar_info_utils.ParseJarInfoFile(jar_path + '.info'))
    else:
      with zipfile.ZipFile(jar_path) as zip_info:
        for path in zip_info.namelist():
          fully_qualified_name = _FullJavaNameFromClassFilePath(path)
          if fully_qualified_name:
            info_data[fully_qualified_name] = '{}/{}'.format(jar_path, path)

  jar_info_utils.WriteJarInfoFile(output, info_data)


def main(args):
  args = build_utils.ExpandFileArgs(args)
  parser = argparse.ArgumentParser(description=__doc__)
  build_utils.AddDepfileOption(parser)
  parser.add_argument('--output', required=True,
                      help='Output .apk.jar.info file')
  parser.add_argument('--apk-jar-file', required=True,
                      help='Path to main .jar file for this APK.')
  parser.add_argument('--dep-jar-files', required=True,
                      help='GN-list of dependent .jar file paths')

  options = parser.parse_args(args)
  options.dep_jar_files = build_utils.ParseGnList(options.dep_jar_files)
  jar_files = [ options.apk_jar_file ] + options.dep_jar_files

  def _OnStaleMd5():
    with tempfile.NamedTemporaryFile() as tmp_file:
      _MergeInfoFiles(tmp_file.name, jar_files)
      shutil.move(tmp_file.name, options.output)
      tmp_file.delete = False

  build_utils.CallAndWriteDepfileIfStale(
      _OnStaleMd5, options,
      input_paths=jar_files,
      output_paths=[options.output],
      depfile_deps=jar_files,
      add_pydeps=False)


if __name__ == '__main__':
  main(sys.argv[1:])
