#!/usr/bin/env vpython
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import sys
import unittest

from pylib.utils import test_filter

class ParseFilterFileTest(unittest.TestCase):

  def testParseFilterFile_commentsAndBlankLines(self):
    input_lines = [
      'positive1',
      '# comment',
      'positive2  # Another comment',
      ''
      'positive3'
    ]
    actual = test_filter.ParseFilterFile(input_lines)
    expected = 'positive1:positive2:positive3'
    self.assertEquals(expected, actual)

  def testParseFilterFile_onlyPositive(self):
    input_lines = [
      'positive1',
      'positive2'
    ]
    actual = test_filter.ParseFilterFile(input_lines)
    expected = 'positive1:positive2'
    self.assertEquals(expected, actual)

  def testParseFilterFile_onlyNegative(self):
    input_lines = [
      '-negative1',
      '-negative2'
    ]
    actual = test_filter.ParseFilterFile(input_lines)
    expected = '-negative1:negative2'
    self.assertEquals(expected, actual)

  def testParseFilterFile_positiveAndNegative(self):
    input_lines = [
      'positive1',
      'positive2',
      '-negative1',
      '-negative2'
    ]
    actual = test_filter.ParseFilterFile(input_lines)
    expected = 'positive1:positive2-negative1:negative2'
    self.assertEquals(expected, actual)


if __name__ == '__main__':
  sys.exit(unittest.main())
