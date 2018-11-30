#!/usr/bin/env python
#
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import optparse
import os
import shutil
import sys
import tempfile

from util import build_utils


def Jar(class_files, classes_dir, jar_path, manifest_file=None,
        provider_configurations=None, additional_files=None):
  jar_path = os.path.abspath(jar_path)

  # The paths of the files in the jar will be the same as they are passed in to
  # the command. Because of this, the command should be run in
  # options.classes_dir so the .class file paths in the jar are correct.
  jar_cwd = classes_dir
  class_files_rel = [os.path.relpath(f, jar_cwd) for f in class_files]
  with tempfile.NamedTemporaryFile(suffix='.jar') as tmp_jar:
    jar_cmd = ['jar', 'cf0', tmp_jar.name]
    if manifest_file:
      jar_cmd[1] += 'm'
      jar_cmd.append(os.path.abspath(manifest_file))
    jar_cmd.extend(class_files_rel)

    for filepath, jar_filepath in additional_files or []:
      full_jar_filepath = os.path.join(jar_cwd, jar_filepath)
      jar_dir = os.path.dirname(full_jar_filepath)
      if not os.path.exists(jar_dir):
        os.makedirs(jar_dir)
      # Some of our JARs are mode 0440 because they exist in the source tree as
      # symlinks to JARs managed by CIPD. shutil.copyfile copies the contents,
      # not the permissions, so the resulting copy is writeable despite the
      # the source JAR not being so. (shutil.copy does copy the permissions and
      # as such doesn't work without changing the mode after.)
      shutil.copyfile(filepath, full_jar_filepath)
      jar_cmd.append(jar_filepath)

    if provider_configurations:
      service_dir = os.path.join(jar_cwd, 'META-INF', 'services')
      if not os.path.exists(service_dir):
        os.makedirs(service_dir)
      for config in provider_configurations:
        config_jar_path = os.path.join(service_dir, os.path.basename(config))
        shutil.copy(config, config_jar_path)
        jar_cmd.append(os.path.relpath(config_jar_path, jar_cwd))

    if not class_files_rel:
      empty_file = os.path.join(classes_dir, '.empty')
      build_utils.Touch(empty_file)
      jar_cmd.append(os.path.relpath(empty_file, jar_cwd))
    build_utils.CheckOutput(jar_cmd, cwd=jar_cwd)

    # Zeros out timestamps so that builds are hermetic.
    build_utils.MergeZips(jar_path, [tmp_jar.name])


def JarDirectory(classes_dir, jar_path, manifest_file=None, predicate=None,
                 provider_configurations=None, additional_files=None):
  all_classes = sorted(build_utils.FindInDirectory(classes_dir, '*.class'))
  if predicate:
    all_classes = [
        f for f in all_classes if predicate(os.path.relpath(f, classes_dir))]

  Jar(all_classes, classes_dir, jar_path, manifest_file=manifest_file,
      provider_configurations=provider_configurations,
      additional_files=additional_files)


def _CreateFilterPredicate(excluded_classes, included_classes):
  if not excluded_classes and not included_classes:
    return None

  def predicate(f):
    # Exclude filters take precidence over include filters.
    if build_utils.MatchesGlob(f, excluded_classes):
      return False
    if included_classes and not build_utils.MatchesGlob(f, included_classes):
      return False
    return True

  return predicate


# TODO(agrieve): Change components/cronet/android/BUILD.gn to use filter_zip.py
#     and delete main().
def main():
  parser = optparse.OptionParser()
  parser.add_option('--classes-dir', help='Directory containing .class files.')
  parser.add_option('--jar-path', help='Jar output path.')
  parser.add_option('--excluded-classes',
      help='GN list of .class file patterns to exclude from the jar.')
  parser.add_option('--included-classes',
      help='GN list of .class file patterns to include in the jar.')

  args = build_utils.ExpandFileArgs(sys.argv[1:])
  options, _ = parser.parse_args(args)

  excluded_classes = []
  if options.excluded_classes:
    excluded_classes = build_utils.ParseGnList(options.excluded_classes)
  included_classes = []
  if options.included_classes:
    included_classes = build_utils.ParseGnList(options.included_classes)

  predicate = _CreateFilterPredicate(excluded_classes, included_classes)
  JarDirectory(options.classes_dir, options.jar_path, predicate=predicate)


if __name__ == '__main__':
  sys.exit(main())
