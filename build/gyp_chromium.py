# Copyright (c) 2012 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""This script is now only used by the closure_compilation builders."""

import argparse
import glob
import gyp_environment
import os
import shlex
import sys

script_dir = os.path.dirname(os.path.realpath(__file__))
chrome_src = os.path.abspath(os.path.join(script_dir, os.pardir))

sys.path.insert(0, os.path.join(chrome_src, 'tools', 'gyp', 'pylib'))
import gyp


def ProcessGypDefinesItems(items):
  """Converts a list of strings to a list of key-value pairs."""
  result = []
  for item in items:
    tokens = item.split('=', 1)
    # Some GYP variables have hyphens, which we don't support.
    if len(tokens) == 2:
      result += [(tokens[0], tokens[1])]
    else:
      # No value supplied, treat it as a boolean and set it. Note that we
      # use the string '1' here so we have a consistent definition whether
      # you do 'foo=1' or 'foo'.
      result += [(tokens[0], '1')]
  return result


def GetSupplementalFiles():
  return []


def GetGypVars(_):
  """Returns a dictionary of all GYP vars."""
  # GYP defines from the environment.
  env_items = ProcessGypDefinesItems(
      shlex.split(os.environ.get('GYP_DEFINES', '')))

  # GYP defines from the command line.
  parser = argparse.ArgumentParser()
  parser.add_argument('-D', dest='defines', action='append', default=[])
  cmdline_input_items = parser.parse_known_args()[0].defines
  cmdline_items = ProcessGypDefinesItems(cmdline_input_items)

  return dict(env_items + cmdline_items)


def main():
  gyp_environment.SetEnvironment()

  print 'Updating projects from gyp files...'
  sys.stdout.flush()
  sys.exit(gyp.main(sys.argv[1:] + [
      '--check',
      '--no-circular-check',
      '-I', os.path.join(script_dir, 'common.gypi'),
      '-D', 'gyp_output_dir=out']))

if __name__ == '__main__':
  sys.exit(main())
