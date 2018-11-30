#!/usr/bin/env python
#
# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Merges a list of jars into a single jar."""

import argparse
import sys

from util import build_utils


def main(args):
  args = build_utils.ExpandFileArgs(args)
  parser = argparse.ArgumentParser()
  build_utils.AddDepfileOption(parser)
  parser.add_argument('--output', required=True, help='Path to output jar.')
  parser.add_argument('--jars', required=True, help='GN list of jar inputs.')
  options = parser.parse_args(args)

  input_jars = build_utils.ParseGnList(options.jars)
  build_utils.MergeZips(options.output, input_jars)

  if options.depfile:
    build_utils.WriteDepfile(options.depfile, options.output, input_jars,
                             add_pydeps=False)


if __name__ == '__main__':
  main(sys.argv[1:])
