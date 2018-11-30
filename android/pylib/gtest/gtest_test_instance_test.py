#!/usr/bin/env python
# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import unittest

from pylib.base import base_test_result
from pylib.gtest import gtest_test_instance


class GtestTestInstanceTests(unittest.TestCase):

  def testParseGTestListTests_simple(self):
    raw_output = [
      'TestCaseOne.',
      '  testOne',
      '  testTwo',
      'TestCaseTwo.',
      '  testThree',
      '  testFour',
    ]
    actual = gtest_test_instance.ParseGTestListTests(raw_output)
    expected = [
      'TestCaseOne.testOne',
      'TestCaseOne.testTwo',
      'TestCaseTwo.testThree',
      'TestCaseTwo.testFour',
    ]
    self.assertEqual(expected, actual)

  def testParseGTestListTests_typeParameterized_old(self):
    raw_output = [
      'TPTestCase/WithTypeParam/0.',
      '  testOne',
      '  testTwo',
    ]
    actual = gtest_test_instance.ParseGTestListTests(raw_output)
    expected = [
      'TPTestCase/WithTypeParam/0.testOne',
      'TPTestCase/WithTypeParam/0.testTwo',
    ]
    self.assertEqual(expected, actual)

  def testParseGTestListTests_typeParameterized_new(self):
    raw_output = [
      'TPTestCase/WithTypeParam/0.  # TypeParam = TypeParam0',
      '  testOne',
      '  testTwo',
    ]
    actual = gtest_test_instance.ParseGTestListTests(raw_output)
    expected = [
      'TPTestCase/WithTypeParam/0.testOne',
      'TPTestCase/WithTypeParam/0.testTwo',
    ]
    self.assertEqual(expected, actual)

  def testParseGTestListTests_valueParameterized_old(self):
    raw_output = [
      'VPTestCase.',
      '  testWithValueParam/0',
      '  testWithValueParam/1',
    ]
    actual = gtest_test_instance.ParseGTestListTests(raw_output)
    expected = [
      'VPTestCase.testWithValueParam/0',
      'VPTestCase.testWithValueParam/1',
    ]
    self.assertEqual(expected, actual)

  def testParseGTestListTests_valueParameterized_new(self):
    raw_output = [
      'VPTestCase.',
      '  testWithValueParam/0  # GetParam() = 0',
      '  testWithValueParam/1  # GetParam() = 1',
    ]
    actual = gtest_test_instance.ParseGTestListTests(raw_output)
    expected = [
      'VPTestCase.testWithValueParam/0',
      'VPTestCase.testWithValueParam/1',
    ]
    self.assertEqual(expected, actual)

  def testParseGTestListTests_emptyTestName(self):
    raw_output = [
      'TestCase.',
      '  ',
      '  nonEmptyTestName',
    ]
    actual = gtest_test_instance.ParseGTestListTests(raw_output)
    expected = [
      'TestCase.nonEmptyTestName',
    ]
    self.assertEqual(expected, actual)

  def testParseGTestOutput_pass(self):
    raw_output = [
      '[ RUN      ] FooTest.Bar',
      '[       OK ] FooTest.Bar (1 ms)',
    ]
    actual = gtest_test_instance.ParseGTestOutput(raw_output, None, None)
    self.assertEquals(1, len(actual))
    self.assertEquals('FooTest.Bar', actual[0].GetName())
    self.assertEquals(1, actual[0].GetDuration())
    self.assertEquals(base_test_result.ResultType.PASS, actual[0].GetType())

  def testParseGTestOutput_fail(self):
    raw_output = [
      '[ RUN      ] FooTest.Bar',
      '[   FAILED ] FooTest.Bar (1 ms)',
    ]
    actual = gtest_test_instance.ParseGTestOutput(raw_output, None, None)
    self.assertEquals(1, len(actual))
    self.assertEquals('FooTest.Bar', actual[0].GetName())
    self.assertEquals(1, actual[0].GetDuration())
    self.assertEquals(base_test_result.ResultType.FAIL, actual[0].GetType())

  def testParseGTestOutput_crash(self):
    raw_output = [
      '[ RUN      ] FooTest.Bar',
      '[  CRASHED ] FooTest.Bar (1 ms)',
    ]
    actual = gtest_test_instance.ParseGTestOutput(raw_output, None, None)
    self.assertEquals(1, len(actual))
    self.assertEquals('FooTest.Bar', actual[0].GetName())
    self.assertEquals(1, actual[0].GetDuration())
    self.assertEquals(base_test_result.ResultType.CRASH, actual[0].GetType())

  def testParseGTestOutput_errorCrash(self):
    raw_output = [
      '[ RUN      ] FooTest.Bar',
      '[ERROR:blah] Currently running: FooTest.Bar',
    ]
    actual = gtest_test_instance.ParseGTestOutput(raw_output, None, None)
    self.assertEquals(1, len(actual))
    self.assertEquals('FooTest.Bar', actual[0].GetName())
    self.assertEquals(0, actual[0].GetDuration())
    self.assertEquals(base_test_result.ResultType.CRASH, actual[0].GetType())

  def testParseGTestOutput_unknown(self):
    raw_output = [
      '[ RUN      ] FooTest.Bar',
    ]
    actual = gtest_test_instance.ParseGTestOutput(raw_output, None, None)
    self.assertEquals(1, len(actual))
    self.assertEquals('FooTest.Bar', actual[0].GetName())
    self.assertEquals(0, actual[0].GetDuration())
    self.assertEquals(base_test_result.ResultType.UNKNOWN, actual[0].GetType())

  def testParseGTestOutput_nonterminalUnknown(self):
    raw_output = [
      '[ RUN      ] FooTest.Bar',
      '[ RUN      ] FooTest.Baz',
      '[       OK ] FooTest.Baz (1 ms)',
    ]
    actual = gtest_test_instance.ParseGTestOutput(raw_output, None, None)
    self.assertEquals(2, len(actual))

    self.assertEquals('FooTest.Bar', actual[0].GetName())
    self.assertEquals(0, actual[0].GetDuration())
    self.assertEquals(base_test_result.ResultType.UNKNOWN, actual[0].GetType())

    self.assertEquals('FooTest.Baz', actual[1].GetName())
    self.assertEquals(1, actual[1].GetDuration())
    self.assertEquals(base_test_result.ResultType.PASS, actual[1].GetType())

  def testParseGTestOutput_deathTestCrashOk(self):
    raw_output = [
      '[ RUN      ] FooTest.Bar',
      '[ CRASHED      ]',
      '[       OK ] FooTest.Bar (1 ms)',
    ]
    actual = gtest_test_instance.ParseGTestOutput(raw_output, None, None)
    self.assertEquals(1, len(actual))

    self.assertEquals('FooTest.Bar', actual[0].GetName())
    self.assertEquals(1, actual[0].GetDuration())
    self.assertEquals(base_test_result.ResultType.PASS, actual[0].GetType())

  def testParseGTestXML_none(self):
    actual = gtest_test_instance.ParseGTestXML(None)
    self.assertEquals([], actual)

  def testTestNameWithoutDisabledPrefix_disabled(self):
    test_name_list = [
      'A.DISABLED_B',
      'DISABLED_A.B',
      'DISABLED_A.DISABLED_B',
    ]
    for test_name in test_name_list:
      actual = gtest_test_instance \
          .TestNameWithoutDisabledPrefix(test_name)
      expected = 'A.B'
      self.assertEquals(expected, actual)

  def testTestNameWithoutDisabledPrefix_flaky(self):
    test_name_list = [
      'A.FLAKY_B',
      'FLAKY_A.B',
      'FLAKY_A.FLAKY_B',
    ]
    for test_name in test_name_list:
      actual = gtest_test_instance \
          .TestNameWithoutDisabledPrefix(test_name)
      expected = 'A.B'
      self.assertEquals(expected, actual)

  def testTestNameWithoutDisabledPrefix_notDisabledOrFlaky(self):
    test_name = 'A.B'
    actual = gtest_test_instance \
        .TestNameWithoutDisabledPrefix(test_name)
    expected = 'A.B'
    self.assertEquals(expected, actual)


if __name__ == '__main__':
  unittest.main(verbosity=2)
