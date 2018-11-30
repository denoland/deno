#!/usr/bin/env vpython
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Tests for local_device_instrumentation_test_run."""

# pylint: disable=protected-access

import unittest

from pylib.base import base_test_result
from pylib.base import mock_environment
from pylib.base import mock_test_instance
from pylib.local.device import local_device_instrumentation_test_run

class LocalDeviceInstrumentationTestRunTest(unittest.TestCase):

  # TODO(crbug.com/797002): Decide whether the _ShouldRetry hook is worth
  # retaining and remove these tests if not.

  def testShouldRetry_failure(self):
    env = mock_environment.MockEnvironment()
    ti = mock_test_instance.MockTestInstance()
    obj = (local_device_instrumentation_test_run
           .LocalDeviceInstrumentationTestRun(env, ti))
    test = {
        'annotations': {},
        'class': 'SadTest',
        'method': 'testFailure',
        'is_junit4': True,
    }
    result = base_test_result.BaseTestResult(
        'SadTest.testFailure', base_test_result.ResultType.FAIL)
    self.assertTrue(obj._ShouldRetry(test, result))

  def testShouldRetry_retryOnFailure(self):
    env = mock_environment.MockEnvironment()
    ti = mock_test_instance.MockTestInstance()
    obj = (local_device_instrumentation_test_run
           .LocalDeviceInstrumentationTestRun(env, ti))
    test = {
        'annotations': {'RetryOnFailure': None},
        'class': 'SadTest',
        'method': 'testRetryOnFailure',
        'is_junit4': True,
    }
    result = base_test_result.BaseTestResult(
        'SadTest.testRetryOnFailure', base_test_result.ResultType.FAIL)
    self.assertTrue(obj._ShouldRetry(test, result))

  def testShouldRetry_notRun(self):
    env = mock_environment.MockEnvironment()
    ti = mock_test_instance.MockTestInstance()
    obj = (local_device_instrumentation_test_run
           .LocalDeviceInstrumentationTestRun(env, ti))
    test = {
        'annotations': {},
        'class': 'SadTest',
        'method': 'testNotRun',
        'is_junit4': True,
    }
    result = base_test_result.BaseTestResult(
        'SadTest.testNotRun', base_test_result.ResultType.NOTRUN)
    self.assertTrue(obj._ShouldRetry(test, result))


if __name__ == '__main__':
  unittest.main(verbosity=2)
