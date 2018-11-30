#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Wraps bin/helper/java_bytecode_rewriter and expands @FileArgs."""

import argparse
import os
import sys

from util import build_utils


def _AddSwitch(parser, val):
  parser.add_argument(
      val, action='store_const', default='--disabled', const=val)


def main(argv):
  argv = build_utils.ExpandFileArgs(argv[1:])
  parser = argparse.ArgumentParser()
  parser.add_argument('--script', required=True,
                      help='Path to the java binary wrapper script.')
  parser.add_argument('--input-jar', required=True)
  parser.add_argument('--output-jar', required=True)
  parser.add_argument('--extra-classpath-jar', dest='extra_jars',
                      action='append', default=[],
                      help='Extra inputs, passed last to the binary script.')
  _AddSwitch(parser, '--enable-custom-resources')
  _AddSwitch(parser, '--enable-assert')
  _AddSwitch(parser, '--enable-thread-annotations')
  args = parser.parse_args(argv)
  extra_classpath_jars = []
  for a in args.extra_jars:
    extra_classpath_jars.extend(build_utils.ParseGnList(a))

  cmd = [args.script, args.input_jar, args.output_jar, args.enable_assert,
         args.enable_custom_resources,
         args.enable_thread_annotations] + extra_classpath_jars
  build_utils.CheckOutput(cmd)


if __name__ == '__main__':
  sys.exit(main(sys.argv))
