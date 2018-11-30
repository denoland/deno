#! /usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# pylint: disable=protected-access

import unittest

from pylib.base import output_manager
from pylib.base import output_manager_test_case
from pylib.constants import host_paths
from pylib.output import remote_output_manager

with host_paths.SysPath(host_paths.PYMOCK_PATH):
  import mock  # pylint: disable=import-error


@mock.patch('pylib.utils.google_storage_helper')
class RemoteOutputManagerTest(output_manager_test_case.OutputManagerTestCase):

  def setUp(self):
    self._output_manager = remote_output_manager.RemoteOutputManager(
        'this-is-a-fake-bucket')

  def testUsableTempFile(self, google_storage_helper_mock):
    del google_storage_helper_mock
    self.assertUsableTempFile(
        self._output_manager._CreateArchivedFile(
            'test_file', 'test_subdir', output_manager.Datatype.TEXT))


if __name__ == '__main__':
  unittest.main()
