#!/usr/bin/env python
#
# Copyright (c) 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Extracts a bundle module's classes from jar created in the synchronized
proguarding step and packages them into a new jar.

Synchronized proguarding means that, when several app modules are combined into
an app bundle, all un-optimized jars for all modules are grouped and sent to a
single proguard command, which generates a single, common, intermediate
optimized jar, and its mapping file.

This script is used to extract, from this synchronized proguard jar, all the
optimized classes corresponding to a single module, into a new .jar file. The
latter will be compiled later into the module's dex file.

For this, the script reads the module's un-obfuscated class names from the
module's unoptimized jars. Then, it maps those to obfuscated class names using
the proguard mapping file. Finally, it extracts the module's class files from
the proguarded jar and zips them into a new module jar. """

import argparse
import os
import sys
import zipfile

from util import build_utils

MANIFEST = """Manifest-Version: 1.0
Created-By: generate_proguarded_module_jar.py
"""


# TODO(tiborg): Share with merge_jar_info_files.py.
def _FullJavaNameFromClassFilePath(path):
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


def main(args):
  args = build_utils.ExpandFileArgs(args)
  parser = argparse.ArgumentParser()
  build_utils.AddDepfileOption(parser)
  parser.add_argument(
      '--proguarded-jar',
      required=True,
      help='Path to input jar produced by synchronized proguarding')
  parser.add_argument(
      '--proguard-mapping',
      required=True,
      help='Path to input proguard mapping produced by synchronized '
      'proguarding')
  parser.add_argument(
      '--module-input-jars',
      required=True,
      help='GN-list of input paths to un-optimized jar files for the current '
      'module. The optimized versions of their .class files will go into '
      'the output jar.')
  parser.add_argument(
      '--output-jar',
      required=True,
      help='Path to output jar file containing the module\'s optimized class '
      'files')
  parser.add_argument(
      '--is-base-module',
      action='store_true',
      help='Inidcates to extract class files for a base module')
  options = parser.parse_args(args)
  options.module_input_jars = build_utils.ParseGnList(options.module_input_jars)

  # Read class names of the currently processed module.
  classes = set()
  for module_jar in options.module_input_jars:
    with zipfile.ZipFile(module_jar) as zip_info:
      for path in zip_info.namelist():
        fully_qualified_name = _FullJavaNameFromClassFilePath(path)
        if fully_qualified_name:
          classes.add(fully_qualified_name)

  # Parse the proguarding mapping to be able to map un-obfuscated to obfuscated
  # names.
  # Proguard mapping files have the following format:
  #
  # {un-obfuscated class name 1} -> {obfuscated class name 1}:
  #     {un-obfuscated member name 1} -> {obfuscated member name 1}
  #     ...
  # {un-obfuscated class name 2} -> {obfuscated class name 2}:
  #     ...
  # ...
  obfuscation_map = {}
  with open(options.proguard_mapping, 'r') as proguard_mapping_file:
    for line in proguard_mapping_file:
      # Skip indented lines since they map member names and not class names.
      if line.startswith(' '):
        continue
      line = line.strip()
      # Skip empty lines.
      if not line:
        continue
      assert line.endswith(':')
      full, obfuscated = line.strip(':').split(' -> ')
      assert full
      assert obfuscated
      obfuscation_map[full] = obfuscated

  # Collect the obfuscated names of classes, which should go into the currently
  # processed module.
  obfuscated_module_classes = set(
      obfuscation_map[c] for c in classes if c in obfuscation_map)

  # Collect horizontally merged classes to later make sure that those only go
  # into the base module. Merging classes horizontally means that proguard took
  # two classes that don't inherit from each other and merged them into one.
  horiz_merged_classes = set()
  obfuscated_classes = sorted(obfuscation_map.values())
  prev_obfuscated_class = None
  for obfuscated_class in obfuscated_classes:
    if prev_obfuscated_class and obfuscated_class == prev_obfuscated_class:
      horiz_merged_classes.add(obfuscated_class)
    prev_obfuscated_class = obfuscated_class

  # Move horizontally merged classes into the base module.
  if options.is_base_module:
    obfuscated_module_classes |= horiz_merged_classes
  else:
    obfuscated_module_classes -= horiz_merged_classes

  # Extract module class files from proguarded jar and store them in a module
  # split jar.
  with zipfile.ZipFile(
      os.path.abspath(options.output_jar), 'w',
      zipfile.ZIP_DEFLATED) as output_jar:
    with zipfile.ZipFile(os.path.abspath(options.proguarded_jar),
                         'r') as proguarded_jar:
      for obfuscated_class in obfuscated_module_classes:
        class_path = obfuscated_class.replace('.', '/') + '.class'
        class_file_content = proguarded_jar.read(class_path)
        output_jar.writestr(class_path, class_file_content)
    output_jar.writestr('META-INF/MANIFEST.MF', MANIFEST)

  if options.depfile:
    build_utils.WriteDepfile(
        options.depfile, options.output_jar, options.module_input_jars +
        [options.proguard_mapping, options.proguarded_jar], add_pydeps=False)


if __name__ == '__main__':
  main(sys.argv[1:])
