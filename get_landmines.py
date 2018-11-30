#!/usr/bin/env python
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""
This file emits the list of reasons why a particular build needs to be clobbered
(or a list of 'landmines').
"""

import sys

import landmine_utils


host_os = landmine_utils.host_os


def print_landmines():
  """
  ALL LANDMINES ARE EMITTED FROM HERE.
  """
  # DO NOT add landmines as part of a regular CL. Landmines are a last-effort
  # bandaid fix if a CL that got landed has a build dependency bug and all bots
  # need to be cleaned up. If you're writing a new CL that causes build
  # dependency problems, fix the dependency problems instead of adding a
  # landmine.
  #
  # Before adding or changing a landmine consider the consequences of doing so.
  # Doing so will wipe out every output directory on every Chrome developer's
  # machine. This can be particularly problematic on Windows where the directory
  # deletion may well fail (locked files, command prompt in the directory,
  # etc.), and generated .sln and .vcxproj files will be deleted.
  #
  # This output directory deletion will be repeated when going back and forth
  # across the change that added the landmine, adding to the cost. There are
  # usually less troublesome alternatives.

  if host_os() == 'win':
    print 'Compile on cc_unittests fails due to symbols removed in r185063.'
  if host_os() == 'linux':
    print 'Builders switching from make to ninja will clobber on this.'
  if host_os() == 'mac':
    print 'Switching from bundle to unbundled dylib (issue 14743002).'
  if host_os() in ('win', 'mac'):
    print ('Improper dependency for create_nmf.py broke in r240802, '
           'fixed in r240860.')
  if host_os() == 'win':
    print 'Switch to VS2015 Update 3, 14393 SDK'
  print 'Need to clobber everything due to an IDL change in r154579 (blink)'
  print 'Need to clobber everything due to gen file moves in r175513 (Blink)'
  print 'Clobber to get rid of obselete test plugin after r248358'
  print 'Clobber to rebuild GN files for V8'
  print 'Clobber to get rid of stale generated mojom.h files'
  print 'Need to clobber everything due to build_nexe change in nacl r13424'
  print '[chromium-dev] PSA: clobber build needed for IDR_INSPECTOR_* compil...'
  print 'blink_resources.grd changed: crbug.com/400860'
  print 'ninja dependency cycle: crbug.com/408192'
  print 'Clobber to fix missing NaCl gyp dependencies (crbug.com/427427).'
  print 'Another clobber for missing NaCl gyp deps (crbug.com/427427).'
  print 'Clobber to fix GN not picking up increased ID range (crbug.com/444902)'
  print 'Remove NaCl toolchains from the output dir (crbug.com/456902)'
  if host_os() == 'win':
    print 'Clobber to delete stale generated files (crbug.com/510086)'
  if host_os() == 'mac':
    print 'Clobber to get rid of evil libsqlite3.dylib (crbug.com/526208)'
  if host_os() == 'mac':
    print 'Clobber to remove libsystem.dylib. See crbug.com/620075'
  if host_os() == 'mac':
    print 'Clobber to get past mojo gen build error (crbug.com/679607)'
  if host_os() == 'win':
    print 'Clobber Windows to fix strange PCH-not-rebuilt errors.'
  print 'CLobber all to fix GN breakage (crbug.com/736215)'
  print 'The Great Blink mv for source files (crbug.com/768828)'

def main():
  print_landmines()
  return 0


if __name__ == '__main__':
  sys.exit(main())
