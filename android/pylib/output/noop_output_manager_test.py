#! /usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# pylint: disable=protected-access

import unittest

from pylib.base import output_manager
from pylib.base import output_manager_test_case
from pylib.output import noop_output_manager


class NoopOutputManagerTest(output_manager_test_case.OutputManagerTestCase):

  def setUp(self):
    self._output_manager = noop_output_manager.NoopOutputManager()

  def testUsableTempFile(self):
    self.assertUsableTempFile(
        self._output_manager._CreateArchivedFile(
            'test_file', 'test_subdir', output_manager.Datatype.TEXT))


if __name__ == '__main__':
  unittest.main()
