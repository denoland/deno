#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Merge the PGC files generated during the profiling step to the PGD database.

This is required to workaround a flakyness in pgomgr.exe where it can run out
of address space while trying to merge all the PGC files at the same time.
"""

import glob
import json
import optparse
import os
import subprocess
import sys


script_dir = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, os.path.join(script_dir, os.pardir))

import vs_toolchain


# Number of PGC files that should be merged in each iteration, merging all
# the files one by one is really slow but merging more than 10 at a time doesn't
# really seem to impact the total time (when merging 180 files).
#
# Number of pgc merged per iteration  |  Time (in min)
# 1                                   |  27.2
# 10                                  |  12.8
# 20                                  |  12.0
# 30                                  |  11.5
# 40                                  |  11.4
# 50                                  |  11.5
# 60                                  |  11.6
# 70                                  |  11.6
# 80                                  |  11.7
#
# TODO(sebmarchand): Measure the memory usage of pgomgr.exe to see how it get
#     affected by the number of pgc files.
_BATCH_SIZE_DEFAULT = 10


def find_pgomgr(chrome_checkout_dir):
  """Find pgomgr.exe."""
  win_toolchain_json_file = os.path.join(chrome_checkout_dir, 'build',
      'win_toolchain.json')
  if not os.path.exists(win_toolchain_json_file):
    raise Exception('The toolchain JSON file is missing.')
  with open(win_toolchain_json_file) as temp_f:
    toolchain_data = json.load(temp_f)
  if not os.path.isdir(toolchain_data['path']):
    raise Exception('The toolchain JSON file is invalid.')

  # Always use the x64 version of pgomgr (the x86 one doesn't work on the bot's
  # environment).
  pgomgr_dir = None
  if toolchain_data['version'] == '2017':
    vc_tools_root = vs_toolchain.FindVCToolsRoot()
    pgomgr_dir = os.path.join(vc_tools_root, 'HostX64', 'x64')

  pgomgr_path = os.path.join(pgomgr_dir, 'pgomgr.exe')
  if not os.path.exists(pgomgr_path):
    raise Exception('pgomgr.exe is missing from %s.' % pgomgr_dir)

  return pgomgr_path


def merge_pgc_files(pgomgr_path, files, pgd_path):
  """Merge all the pgc_files in |files| to |pgd_path|."""
  merge_command = [
      pgomgr_path,
      '/merge'
  ]
  merge_command.extend(files)
  merge_command.append(pgd_path)
  proc = subprocess.Popen(merge_command, stdout=subprocess.PIPE)
  stdout, _ = proc.communicate()
  print stdout
  return proc.returncode


def main():
  parser = optparse.OptionParser(usage='%prog [options]')
  parser.add_option('--checkout-dir', help='The Chrome checkout directory.')
  parser.add_option('--target-cpu', help='[DEPRECATED] The target\'s bitness.')
  parser.add_option('--build-dir', help='Chrome build directory.')
  parser.add_option('--binary-name', help='The binary for which the PGC files '
                    'should be merged, without extension.')
  parser.add_option('--files-per-iter', help='The number of PGC files to merge '
                    'in each iteration, default to %d.' % _BATCH_SIZE_DEFAULT,
                    type='int', default=_BATCH_SIZE_DEFAULT)
  options, _ = parser.parse_args()

  if not options.checkout_dir:
    parser.error('--checkout-dir is required')
  if not options.build_dir:
    parser.error('--build-dir is required')
  if not options.binary_name:
    parser.error('--binary-name is required')

  # Starts by finding pgomgr.exe.
  pgomgr_path = find_pgomgr(options.checkout_dir)

  pgc_files = glob.glob(os.path.join(options.build_dir,
                                     '%s*.pgc' % options.binary_name))
  pgd_file = os.path.join(options.build_dir, '%s.pgd' % options.binary_name)

  def _split_in_chunks(items, chunk_size):
    """Split |items| in chunks of size |chunk_size|.

    Source: http://stackoverflow.com/a/312464
    """
    for i in xrange(0, len(items), chunk_size):
      yield items[i:i + chunk_size]
  for chunk in _split_in_chunks(pgc_files, options.files_per_iter):
    files_to_merge = []
    for pgc_file in chunk:
      files_to_merge.append(
          os.path.join(options.build_dir, os.path.basename(pgc_file)))
    ret = merge_pgc_files(pgomgr_path, files_to_merge, pgd_file)
    # pgomgr.exe sometimes fails to merge too many files at the same time (it
    # usually complains that a stream is missing, but if you try to merge this
    # file individually it works), try to merge all the PGCs from this batch one
    # at a time instead. Don't fail the build if we can't merge a file.
    # TODO(sebmarchand): Report this to Microsoft, check if this is still
    # happening with VS2017.
    if ret != 0:
      print ('Error while trying to merge several PGC files at the same time, '
             'trying to merge them one by one.')
      for pgc_file in chunk:
        ret = merge_pgc_files(
            pgomgr_path,
            [os.path.join(options.build_dir, os.path.basename(pgc_file))],
            pgd_file
        )
        if ret != 0:
          print 'Error while trying to merge %s, continuing.' % pgc_file


if __name__ == '__main__':
  sys.exit(main())
