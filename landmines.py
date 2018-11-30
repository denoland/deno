#!/usr/bin/env python
# Copyright (c) 2012 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""
This script runs every build as the first hook (See DEPS). If it detects that
the build should be clobbered, it will delete the contents of the build
directory.

A landmine is tripped when a builder checks out a different revision, and the
diff between the new landmines and the old ones is non-null. At this point, the
build is clobbered.

Before adding or changing a landmine consider the consequences of doing so.
Doing so will wipe out every output directory on every Chrome developer's
machine. This can be particularly problematic on Windows where the directory
deletion may well fail (locked files, command prompt in the directory, etc.),
and generated .sln and .vcxproj files will be deleted.

This output directory deletion will be repated when going back and forth across
the change that added the landmine, adding to the cost. There are usually less
troublesome alternatives.
"""

import difflib
import errno
import logging
import optparse
import os
import sys
import subprocess
import time

import clobber
import landmine_utils


def get_build_dir(src_dir):
  """
  Returns output directory absolute path dependent on build and targets.
  Examples:
    r'c:\b\build\slave\win\build\src\out'
    '/mnt/data/b/build/slave/linux/build/src/out'
    '/b/build/slave/ios_rel_device/build/src/out'

  Keep this function in sync with tools/build/scripts/slave/compile.py
  """
  if 'CHROMIUM_OUT_DIR' in os.environ:
    output_dir = os.environ.get('CHROMIUM_OUT_DIR').strip()
    if not output_dir:
      raise Error('CHROMIUM_OUT_DIR environment variable is set but blank!')
  else:
    output_dir = 'out'
  return os.path.abspath(os.path.join(src_dir, output_dir))


def clobber_if_necessary(new_landmines, src_dir):
  """Does the work of setting, planting, and triggering landmines."""
  out_dir = get_build_dir(src_dir)
  landmines_path = os.path.normpath(os.path.join(src_dir, '.landmines'))
  try:
    os.makedirs(out_dir)
  except OSError as e:
    if e.errno == errno.EEXIST:
      pass

  if os.path.exists(landmines_path):
    with open(landmines_path, 'r') as f:
      old_landmines = f.readlines()
    if old_landmines != new_landmines:
      old_date = time.ctime(os.stat(landmines_path).st_ctime)
      diff = difflib.unified_diff(old_landmines, new_landmines,
          fromfile='old_landmines', tofile='new_landmines',
          fromfiledate=old_date, tofiledate=time.ctime(), n=0)
      sys.stdout.write('Clobbering due to:\n')
      sys.stdout.writelines(diff)
      sys.stdout.flush()

      clobber.clobber(out_dir)

  # Save current set of landmines for next time.
  with open(landmines_path, 'w') as f:
    f.writelines(new_landmines)


def process_options():
  """Returns an options object containing the configuration for this script."""
  parser = optparse.OptionParser()
  parser.add_option(
      '-s', '--landmine-scripts', action='append',
      help='Path to the script which emits landmines to stdout. The target '
           'is passed to this script via option -t. Note that an extra '
           'script can be specified via an env var EXTRA_LANDMINES_SCRIPT.')
  parser.add_option('-d', '--src-dir',
      help='Path of the source root dir. Overrides the default location of the '
           'source root dir when calculating the build directory.')
  parser.add_option('-v', '--verbose', action='store_true',
      default=('LANDMINES_VERBOSE' in os.environ),
      help=('Emit some extra debugging information (default off). This option '
          'is also enabled by the presence of a LANDMINES_VERBOSE environment '
          'variable.'))

  options, args = parser.parse_args()

  if args:
    parser.error('Unknown arguments %s' % args)

  logging.basicConfig(
      level=logging.DEBUG if options.verbose else logging.ERROR)

  if options.src_dir:
    if not os.path.isdir(options.src_dir):
      parser.error('Cannot find source root dir at %s' % options.src_dir)
    logging.debug('Overriding source root dir. Using: %s', options.src_dir)
  else:
    options.src_dir = \
        os.path.dirname(os.path.dirname(os.path.realpath(__file__)))

  if not options.landmine_scripts:
    options.landmine_scripts = [os.path.join(options.src_dir, 'build',
                                             'get_landmines.py')]

  extra_script = os.environ.get('EXTRA_LANDMINES_SCRIPT')
  if extra_script:
    options.landmine_scripts += [extra_script]

  return options


def main():
  options = process_options()

  landmines = []
  for s in options.landmine_scripts:
    proc = subprocess.Popen([sys.executable, s], stdout=subprocess.PIPE)
    output, _ = proc.communicate()
    landmines.extend([('%s\n' % l.strip()) for l in output.splitlines()])
  clobber_if_necessary(landmines, options.src_dir)

  return 0


if __name__ == '__main__':
  sys.exit(main())
