#! /usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# pylint: disable=protected-access

import tempfile
import shutil
import unittest

from pylib.base import output_manager
from pylib.base import output_manager_test_case
from pylib.output import local_output_manager


class LocalOutputManagerTest(output_manager_test_case.OutputManagerTestCase):

  def setUp(self):
    self._output_dir = tempfile.mkdtemp()
    self._output_manager = local_output_manager.LocalOutputManager(
        self._output_dir)

  def testUsableTempFile(self):
    self.assertUsableTempFile(
        self._output_manager._CreateArchivedFile(
            'test_file', 'test_subdir', output_manager.Datatype.TEXT))

  def tearDown(self):
    shutil.rmtree(self._output_dir)


if __name__ == '__main__':
  unittest.main()
