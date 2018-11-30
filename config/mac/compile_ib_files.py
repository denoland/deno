# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.


import argparse
import logging
import os
import re
import subprocess
import sys


def main():
  parser = argparse.ArgumentParser(
      description='A script to compile xib and storyboard.',
      fromfile_prefix_chars='@')
  parser.add_argument('-o', '--output', required=True,
                      help='Path to output bundle.')
  parser.add_argument('-i', '--input', required=True,
                      help='Path to input xib or storyboard.')
  parser.add_argument('--developer_dir', required=False,
                      help='Path to Xcode.')
  args, unknown_args = parser.parse_known_args()

  if args.developer_dir:
    os.environ['DEVELOPER_DIR'] = args.developer_dir

  ibtool_args = [
      'xcrun', 'ibtool',
      '--errors', '--warnings', '--notices',
      '--output-format', 'human-readable-text'
  ]
  ibtool_args += unknown_args
  ibtool_args += [
      '--compile',
      os.path.abspath(args.output),
      os.path.abspath(args.input)
  ]

  ibtool_section_re = re.compile(r'/\*.*\*/')
  ibtool_re = re.compile(r'.*note:.*is clipping its content')
  try:
    stdout = subprocess.check_output(ibtool_args)
  except subprocess.CalledProcessError as e:
    print(e.output)
    raise
  current_section_header = None
  for line in stdout.splitlines():
    if ibtool_section_re.match(line):
      current_section_header = line
    elif not ibtool_re.match(line):
      if current_section_header:
        print(current_section_header)
        current_section_header = None
      print(line)
  return 0


if __name__ == '__main__':
  sys.exit(main())
