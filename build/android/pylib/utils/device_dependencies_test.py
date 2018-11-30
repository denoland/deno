#! /usr/bin/env python
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
import unittest

from pylib import constants
from pylib.utils import device_dependencies


class DevicePathComponentsForTest(unittest.TestCase):

  def testCheckedInFile(self):
    test_path = os.path.join(constants.DIR_SOURCE_ROOT, 'foo', 'bar', 'baz.txt')
    output_directory = os.path.join(
        constants.DIR_SOURCE_ROOT, 'out-foo', 'Release')
    self.assertEquals(
        [None, 'foo', 'bar', 'baz.txt'],
        device_dependencies.DevicePathComponentsFor(
            test_path, output_directory))

  def testOutputDirectoryFile(self):
    test_path = os.path.join(constants.DIR_SOURCE_ROOT, 'out-foo', 'Release',
                             'icudtl.dat')
    output_directory = os.path.join(
        constants.DIR_SOURCE_ROOT, 'out-foo', 'Release')
    self.assertEquals(
        [None, 'icudtl.dat'],
        device_dependencies.DevicePathComponentsFor(
            test_path, output_directory))

  def testOutputDirectorySubdirFile(self):
    test_path = os.path.join(constants.DIR_SOURCE_ROOT, 'out-foo', 'Release',
                             'test_dir', 'icudtl.dat')
    output_directory = os.path.join(
        constants.DIR_SOURCE_ROOT, 'out-foo', 'Release')
    self.assertEquals(
        [None, 'test_dir', 'icudtl.dat'],
        device_dependencies.DevicePathComponentsFor(
            test_path, output_directory))

  def testOutputDirectoryPakFile(self):
    test_path = os.path.join(constants.DIR_SOURCE_ROOT, 'out-foo', 'Release',
                             'foo.pak')
    output_directory = os.path.join(
        constants.DIR_SOURCE_ROOT, 'out-foo', 'Release')
    self.assertEquals(
        [None, 'paks', 'foo.pak'],
        device_dependencies.DevicePathComponentsFor(
            test_path, output_directory))


if __name__ == '__main__':
  unittest.main()
