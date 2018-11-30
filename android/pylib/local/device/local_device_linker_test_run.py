# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging
import sys
import traceback

from pylib.base import base_test_result
from pylib.linker import test_case
from pylib.local.device import local_device_environment
from pylib.local.device import local_device_test_run


class LinkerExceptionTestResult(base_test_result.BaseTestResult):
  """Test result corresponding to a python exception in a host-custom test."""

  def __init__(self, test_name, exc_info):
    """Constructs a LinkerExceptionTestResult object.

    Args:
      test_name: name of the test which raised an exception.
      exc_info: exception info, ostensibly from sys.exc_info().
    """
    exc_type, exc_value, exc_traceback = exc_info
    trace_info = ''.join(traceback.format_exception(exc_type, exc_value,
                                                    exc_traceback))
    log_msg = 'Exception:\n' + trace_info

    super(LinkerExceptionTestResult, self).__init__(
        test_name,
        base_test_result.ResultType.FAIL,
        log="%s %s" % (exc_type, log_msg))


class LocalDeviceLinkerTestRun(local_device_test_run.LocalDeviceTestRun):

  def _CreateShards(self, tests):
    return tests

  def _GetTests(self):
    return self._test_instance.GetTests()

  def _GetUniqueTestName(self, test):
    return test.qualified_name

  def _RunTest(self, device, test):
    assert isinstance(test, test_case.LinkerTestCaseBase)

    try:
      result = test.Run(device)
    except Exception: # pylint: disable=broad-except
      logging.exception('Caught exception while trying to run test: ' +
                        test.tagged_name)
      exc_info = sys.exc_info()
      result = LinkerExceptionTestResult(test.tagged_name, exc_info)

    return result, None

  def SetUp(self):
    @local_device_environment.handle_shard_failures_with(
        on_failure=self._env.BlacklistDevice)
    def individual_device_set_up(dev):
      dev.Install(self._test_instance.test_apk)

    self._env.parallel_devices.pMap(individual_device_set_up)

  def _ShouldShard(self):
    return True

  def TearDown(self):
    pass

  def TestPackage(self):
    pass
