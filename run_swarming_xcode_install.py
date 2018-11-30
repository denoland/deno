#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""
This script runs swarming_xcode_install on the bots.  It should be run when we
need to upgrade all the swarming testers.  It:
  1) Packages two python files into an isolate.
  2) Runs the isolate on swarming machines that satisfy certain dimensions.

Example usage:
  $  ./build/run_swarming_xcode_install.py  --luci_path ~/work/luci-py \
       --swarming-server touch-swarming.appspot.com \
       --isolate-server touch-isolate.appspot.com
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile


def main():
  parser = argparse.ArgumentParser(
      description='Run swarming_xcode_install on the bots.')
  parser.add_argument('--luci_path', required=True, type=os.path.abspath)
  parser.add_argument('--swarming-server', required=True, type=str)
  parser.add_argument('--isolate-server', required=True, type=str)
  parser.add_argument('--batches', type=int, default=25,
                      help="Run xcode install in batches of size |batches|.")
  parser.add_argument('--dimension', nargs=2, action='append')
  args = parser.parse_args()

  args.dimension = args.dimension or []

  script_dir = os.path.dirname(os.path.abspath(__file__))
  tmp_dir = tempfile.mkdtemp(prefix='swarming_xcode')
  try:
    print 'Making isolate.'
    shutil.copyfile(os.path.join(script_dir, 'swarming_xcode_install.py'),
                    os.path.join(tmp_dir, 'swarming_xcode_install.py'))
    shutil.copyfile(os.path.join(script_dir, 'mac_toolchain.py'),
                    os.path.join(tmp_dir, 'mac_toolchain.py'))

    luci_client = os.path.join(args.luci_path, 'client')
    cmd = [
      sys.executable, os.path.join(luci_client, 'isolateserver.py'), 'archive',
      '-I', args.isolate_server, tmp_dir,
    ]
    isolate_hash = subprocess.check_output(cmd).split()[0]

    print 'Running swarming_xcode_install.'
    # TODO(crbug.com/765361): The dimensions below should be updated once
    # swarming for iOS is fleshed out, likely removing xcode_version 9 and
    # adding different dimensions.
    luci_tools = os.path.join(luci_client, 'tools')
    dimensions = [['pool', 'Chrome'], ['xcode_version', '9.0']] + args.dimension
    dim_args = []
    for d in dimensions:
      dim_args += ['--dimension'] + d
    cmd = [
      sys.executable, os.path.join(luci_tools, 'run_on_bots.py'),
      '--swarming', args.swarming_server, '--isolate-server',
      args.isolate_server, '--priority', '20', '--batches', str(args.batches),
      '--tags', 'name:run_swarming_xcode_install',
    ] + dim_args + ['--name', 'run_swarming_xcode_install', '--', isolate_hash,
      'python', 'swarming_xcode_install.py',
    ]
    subprocess.check_call(cmd)
    print 'All tasks completed.'

  finally:
    shutil.rmtree(tmp_dir)
  return 0


if __name__ == '__main__':
  sys.exit(main())
