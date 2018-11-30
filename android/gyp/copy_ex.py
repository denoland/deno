#!/usr/bin/env python
#
# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Copies files to a directory."""

import filecmp
import itertools
import optparse
import os
import shutil
import sys

from util import build_utils


def _get_all_files(base):
  """Returns a list of all the files in |base|. Each entry is relative to the
  last path entry of |base|."""
  result = []
  dirname = os.path.dirname(base)
  for root, _, files in os.walk(base):
    result.extend([os.path.join(root[len(dirname):], f) for f in files])
  return result

def CopyFile(f, dest, deps):
  """Copy file or directory and update deps."""
  if os.path.isdir(f):
    shutil.copytree(f, os.path.join(dest, os.path.basename(f)))
    deps.extend(_get_all_files(f))
  else:
    if os.path.isfile(os.path.join(dest, os.path.basename(f))):
      dest = os.path.join(dest, os.path.basename(f))

    deps.append(f)

    if os.path.isfile(dest):
      if filecmp.cmp(dest, f, shallow=False):
        return
      # The shutil.copy() below would fail if the file does not have write
      # permissions. Deleting the file has similar costs to modifying the
      # permissions.
      os.unlink(dest)

    shutil.copy(f, dest)

def DoCopy(options, deps):
  """Copy files or directories given in options.files and update deps."""
  files = list(itertools.chain.from_iterable(build_utils.ParseGnList(f)
                                             for f in options.files))

  for f in files:
    if os.path.isdir(f) and not options.clear:
      print ('To avoid stale files you must use --clear when copying '
             'directories')
      sys.exit(-1)
    CopyFile(f, options.dest, deps)

def DoRenaming(options, deps):
  """Copy and rename files given in options.renaming_sources and update deps."""
  src_files = list(itertools.chain.from_iterable(
                   build_utils.ParseGnList(f)
                   for f in options.renaming_sources))

  dest_files = list(itertools.chain.from_iterable(
                    build_utils.ParseGnList(f)
                    for f in options.renaming_destinations))

  if (len(src_files) != len(dest_files)):
    print('Renaming source and destination files not match.')
    sys.exit(-1)

  for src, dest in itertools.izip(src_files, dest_files):
    if os.path.isdir(src):
      print ('renaming diretory is not supported.')
      sys.exit(-1)
    else:
      CopyFile(src, os.path.join(options.dest, dest), deps)

def main(args):
  args = build_utils.ExpandFileArgs(args)

  parser = optparse.OptionParser()
  build_utils.AddDepfileOption(parser)

  parser.add_option('--dest', help='Directory to copy files to.')
  parser.add_option('--files', action='append',
                    help='List of files to copy.')
  parser.add_option('--clear', action='store_true',
                    help='If set, the destination directory will be deleted '
                    'before copying files to it. This is highly recommended to '
                    'ensure that no stale files are left in the directory.')
  parser.add_option('--stamp', help='Path to touch on success.')
  parser.add_option('--renaming-sources',
                    action='append',
                    help='List of files need to be renamed while being '
                         'copied to dest directory')
  parser.add_option('--renaming-destinations',
                    action='append',
                    help='List of destination file name without path, the '
                         'number of elements must match rename-sources.')

  options, _ = parser.parse_args(args)

  if options.clear:
    build_utils.DeleteDirectory(options.dest)
    build_utils.MakeDirectory(options.dest)

  deps = []

  if options.files:
    DoCopy(options, deps)

  if options.renaming_sources:
    DoRenaming(options, deps)

  if options.depfile:
    build_utils.WriteDepfile(
        options.depfile, options.stamp, deps, add_pydeps=False)

  if options.stamp:
    build_utils.Touch(options.stamp)


if __name__ == '__main__':
  sys.exit(main(sys.argv[1:]))
