#!/usr/bin/env vpython
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# pylint: disable=protected-access

import unittest

from pylib.base import base_test_result
from pylib.constants import host_paths
from pylib.local.device import local_device_test_run

with host_paths.SysPath(host_paths.PYMOCK_PATH):
  import mock # pylint: disable=import-error


class SubstituteDeviceRootTest(unittest.TestCase):

  def testNoneDevicePath(self):
    self.assertEquals(
        '/fake/device/root',
        local_device_test_run.SubstituteDeviceRoot(
            None, '/fake/device/root'))

  def testStringDevicePath(self):
    self.assertEquals(
        '/another/fake/device/path',
        local_device_test_run.SubstituteDeviceRoot(
            '/another/fake/device/path', '/fake/device/root'))

  def testListWithNoneDevicePath(self):
    self.assertEquals(
        '/fake/device/root/subpath',
        local_device_test_run.SubstituteDeviceRoot(
            [None, 'subpath'], '/fake/device/root'))

  def testListWithoutNoneDevicePath(self):
    self.assertEquals(
        '/another/fake/device/path',
        local_device_test_run.SubstituteDeviceRoot(
            ['/', 'another', 'fake', 'device', 'path'],
            '/fake/device/root'))


class TestLocalDeviceTestRun(local_device_test_run.LocalDeviceTestRun):

  # pylint: disable=abstract-method

  def __init__(self):
    super(TestLocalDeviceTestRun, self).__init__(
        mock.MagicMock(), mock.MagicMock())


class TestLocalDeviceNonStringTestRun(
    local_device_test_run.LocalDeviceTestRun):

  # pylint: disable=abstract-method

  def __init__(self):
    super(TestLocalDeviceNonStringTestRun, self).__init__(
        mock.MagicMock(), mock.MagicMock())

  def _GetUniqueTestName(self, test):
    return test['name']


class LocalDeviceTestRunTest(unittest.TestCase):

  def testGetTestsToRetry_allTestsPassed(self):
    results = [
        base_test_result.BaseTestResult(
            'Test1', base_test_result.ResultType.PASS),
        base_test_result.BaseTestResult(
            'Test2', base_test_result.ResultType.PASS),
    ]

    tests = [r.GetName() for r in results]
    try_results = base_test_result.TestRunResults()
    try_results.AddResults(results)

    test_run = TestLocalDeviceTestRun()
    tests_to_retry = test_run._GetTestsToRetry(tests, try_results)
    self.assertEquals(0, len(tests_to_retry))

  def testGetTestsToRetry_testFailed(self):
    results = [
        base_test_result.BaseTestResult(
            'Test1', base_test_result.ResultType.FAIL),
        base_test_result.BaseTestResult(
            'Test2', base_test_result.ResultType.PASS),
    ]

    tests = [r.GetName() for r in results]
    try_results = base_test_result.TestRunResults()
    try_results.AddResults(results)

    test_run = TestLocalDeviceTestRun()
    tests_to_retry = test_run._GetTestsToRetry(tests, try_results)
    self.assertEquals(1, len(tests_to_retry))
    self.assertIn('Test1', tests_to_retry)

  def testGetTestsToRetry_testUnknown(self):
    results = [
        base_test_result.BaseTestResult(
            'Test2', base_test_result.ResultType.PASS),
    ]

    tests = ['Test1'] + [r.GetName() for r in results]
    try_results = base_test_result.TestRunResults()
    try_results.AddResults(results)

    test_run = TestLocalDeviceTestRun()
    tests_to_retry = test_run._GetTestsToRetry(tests, try_results)
    self.assertEquals(1, len(tests_to_retry))
    self.assertIn('Test1', tests_to_retry)

  def testGetTestsToRetry_wildcardFilter_allPass(self):
    results = [
        base_test_result.BaseTestResult(
            'TestCase.Test1', base_test_result.ResultType.PASS),
        base_test_result.BaseTestResult(
            'TestCase.Test2', base_test_result.ResultType.PASS),
    ]

    tests = ['TestCase.*']
    try_results = base_test_result.TestRunResults()
    try_results.AddResults(results)

    test_run = TestLocalDeviceTestRun()
    tests_to_retry = test_run._GetTestsToRetry(tests, try_results)
    self.assertEquals(0, len(tests_to_retry))

  def testGetTestsToRetry_wildcardFilter_oneFails(self):
    results = [
        base_test_result.BaseTestResult(
            'TestCase.Test1', base_test_result.ResultType.PASS),
        base_test_result.BaseTestResult(
            'TestCase.Test2', base_test_result.ResultType.FAIL),
    ]

    tests = ['TestCase.*']
    try_results = base_test_result.TestRunResults()
    try_results.AddResults(results)

    test_run = TestLocalDeviceTestRun()
    tests_to_retry = test_run._GetTestsToRetry(tests, try_results)
    self.assertEquals(1, len(tests_to_retry))
    self.assertIn('TestCase.*', tests_to_retry)

  def testGetTestsToRetry_nonStringTests(self):
    results = [
        base_test_result.BaseTestResult(
            'TestCase.Test1', base_test_result.ResultType.PASS),
        base_test_result.BaseTestResult(
            'TestCase.Test2', base_test_result.ResultType.FAIL),
    ]

    tests = [
        {'name': 'TestCase.Test1'},
        {'name': 'TestCase.Test2'},
    ]
    try_results = base_test_result.TestRunResults()
    try_results.AddResults(results)

    test_run = TestLocalDeviceNonStringTestRun()
    tests_to_retry = test_run._GetTestsToRetry(tests, try_results)
    self.assertEquals(1, len(tests_to_retry))
    self.assertIsInstance(tests_to_retry[0], dict)
    self.assertEquals(tests[1], tests_to_retry[0])


if __name__ == '__main__':
  unittest.main(verbosity=2)
