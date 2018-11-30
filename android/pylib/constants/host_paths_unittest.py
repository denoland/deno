#!/usr/bin/env python
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging
import os
import unittest

import pylib.constants as constants
import pylib.constants.host_paths as host_paths


# This map corresponds to the binprefix of NDK prebuilt toolchains for various
# target CPU architectures. Note that 'x86_64' and 'x64' are the same.
_EXPECTED_NDK_TOOL_SUBDIR_MAP = {
  'arm': 'toolchains/arm-linux-androideabi-4.9/prebuilt/linux-x86_64/bin/' +
         'arm-linux-androideabi-',
  'arm64':
      'toolchains/aarch64-linux-android-4.9/prebuilt/linux-x86_64/bin/' +
      'aarch64-linux-android-',
  'x86': 'toolchains/x86-4.9/prebuilt/linux-x86_64/bin/i686-linux-android-',
  'x86_64':
      'toolchains/x86_64-4.9/prebuilt/linux-x86_64/bin/x86_64-linux-android-',
  'x64':
      'toolchains/x86_64-4.9/prebuilt/linux-x86_64/bin/x86_64-linux-android-',
   'mips':
      'toolchains/mipsel-linux-android-4.9/prebuilt/linux-x86_64/bin/' +
      'mipsel-linux-android-'
}


class HostPathsTest(unittest.TestCase):
  def setUp(self):
    logging.getLogger().setLevel(logging.ERROR)

  def test_GetAaptPath(self):
    _EXPECTED_AAPT_PATH = os.path.join(constants.ANDROID_SDK_TOOLS, 'aapt')
    self.assertEqual(host_paths.GetAaptPath(), _EXPECTED_AAPT_PATH)
    self.assertEqual(host_paths.GetAaptPath(), _EXPECTED_AAPT_PATH)

  def test_ToolPath(self):
    for cpu_arch, binprefix in _EXPECTED_NDK_TOOL_SUBDIR_MAP.iteritems():
      expected_binprefix = os.path.join(constants.ANDROID_NDK_ROOT, binprefix)
      expected_path = expected_binprefix + 'foo'
      self.assertEqual(host_paths.ToolPath('foo', cpu_arch), expected_path)


if __name__ == '__main__':
  unittest.main()
