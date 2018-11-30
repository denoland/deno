#!/usr/bin/env python
#
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import os
import sys

from util import build_utils


def main():
  args = build_utils.ExpandFileArgs(sys.argv[1:])
  parser = argparse.ArgumentParser()
  parser.add_argument('--desugar-jar', required=True,
                      help='Path to Desugar.jar.')
  parser.add_argument('--input-jar', required=True,
                      help='Jar input path to include .class files from.')
  parser.add_argument('--output-jar', required=True,
                      help='Jar output path.')
  parser.add_argument('--classpath', required=True,
                      help='Classpath.')
  parser.add_argument('--bootclasspath', required=True,
                      help='Path to javac bootclasspath interface jar.')
  options = parser.parse_args(args)

  options.bootclasspath = build_utils.ParseGnList(options.bootclasspath)
  options.classpath = build_utils.ParseGnList(options.classpath)

  cmd = [
      'java',
      '-jar',
      options.desugar_jar,
      '--input',
      options.input_jar,
      '--output',
      options.output_jar,
      # Don't include try-with-resources files in every .jar. Instead, they
      # are included via //third_party/bazel/desugar:desugar_runtime_java.
      '--desugar_try_with_resources_omit_runtime_classes',
  ]
  for path in options.bootclasspath:
    cmd += ['--bootclasspath_entry', path]
  for path in options.classpath:
    cmd += ['--classpath_entry', path]
  build_utils.CheckOutput(cmd, print_stdout=False)


if __name__ == '__main__':
  sys.exit(main())
