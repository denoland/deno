# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os.path
import unittest


class OutputManagerTestCase(unittest.TestCase):

  def assertUsableTempFile(self, archived_tempfile):
    self.assertTrue(bool(archived_tempfile.name))
    self.assertTrue(os.path.exists(archived_tempfile.name))
    self.assertTrue(os.path.isfile(archived_tempfile.name))
