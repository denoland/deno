#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""
Script used to install Xcode on the swarming bots.
"""

import os
import shutil
import subprocess
import sys
import tarfile
import tempfile

import mac_toolchain

VERSION = '9A235'
URL = 'gs://chrome-mac-sdk/ios-toolchain-9A235-1.tgz'
REMOVE_DIR = '/Applications/Xcode9.0-Beta4.app/'
OUTPUT_DIR = '/Applications/Xcode9.0.app/'

def main():
  # Check if it's already installed.
  if os.path.exists(OUTPUT_DIR):
    env = os.environ.copy()
    env['DEVELOPER_DIR'] = OUTPUT_DIR
    cmd = ['xcodebuild', '-version']
    found_version = \
        subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE).communicate()[0]
    if VERSION in found_version:
      print "Xcode %s already installed" % VERSION
      sys.exit(0)

  # Confirm old dir is there first.
  if not os.path.exists(REMOVE_DIR):
    print "Failing early since %s isn't there." % REMOVE_DIR
    sys.exit(1)

  # Download Xcode.
  with tempfile.NamedTemporaryFile() as temp:
    env = os.environ.copy()
    env['PATH'] += ":/b/depot_tools"
    subprocess.check_call(['gsutil.py', 'cp', URL, temp.name], env=env)
    if os.path.exists(OUTPUT_DIR):
      shutil.rmtree(OUTPUT_DIR)
    if not os.path.exists(OUTPUT_DIR):
      os.makedirs(OUTPUT_DIR)
    tarfile.open(mode='r:gz', name=temp.name).extractall(path=OUTPUT_DIR)

  # Accept license, call runFirstLaunch.
  mac_toolchain.FinalizeUnpack(OUTPUT_DIR, 'ios')

  # Set new Xcode as default.
  subprocess.check_call(['sudo', '/usr/bin/xcode-select', '-s', OUTPUT_DIR])

  if os.path.exists(REMOVE_DIR):
    shutil.rmtree(REMOVE_DIR)


if __name__ == '__main__':
  sys.exit(main())

