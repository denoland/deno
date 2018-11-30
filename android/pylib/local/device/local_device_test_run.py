# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import fnmatch
import logging
import posixpath
import signal
import thread
import threading

from devil import base_error
from devil.android import crash_handler
from devil.android import device_errors
from devil.android.sdk import version_codes
from devil.android.tools import device_recovery
from devil.utils import signal_handler
from pylib import valgrind_tools
from pylib.base import base_test_result
from pylib.base import test_run
from pylib.base import test_collection
from pylib.local.device import local_device_environment


_SIGTERM_TEST_LOG = (
  '  Suite execution terminated, probably due to swarming timeout.\n'
  '  Your test may not have run.')


def SubstituteDeviceRoot(device_path, device_root):
  if not device_path:
    return device_root
  elif isinstance(device_path, list):
    return posixpath.join(*(p if p else device_root for p in device_path))
  else:
    return device_path


class TestsTerminated(Exception):
  pass


class InvalidShardingSettings(Exception):
  def __init__(self, shard_index, total_shards):
    super(InvalidShardingSettings, self).__init__(
        'Invalid sharding settings. shard_index: %d total_shards: %d'
            % (shard_index, total_shards))


class LocalDeviceTestRun(test_run.TestRun):

  def __init__(self, env, test_instance):
    super(LocalDeviceTestRun, self).__init__(env, test_instance)
    self._tools = {}

  #override
  def RunTests(self):
    tests = self._GetTests()

    exit_now = threading.Event()

    @local_device_environment.handle_shard_failures
    def run_tests_on_device(dev, tests, results):
      for test in tests:
        if exit_now.isSet():
          thread.exit()

        result = None
        rerun = None
        try:
          result, rerun = crash_handler.RetryOnSystemCrash(
              lambda d, t=test: self._RunTest(d, t),
              device=dev)
          if isinstance(result, base_test_result.BaseTestResult):
            results.AddResult(result)
          elif isinstance(result, list):
            results.AddResults(result)
          else:
            raise Exception(
                'Unexpected result type: %s' % type(result).__name__)
        except device_errors.CommandTimeoutError:
          if isinstance(test, list):
            results.AddResults(
                base_test_result.BaseTestResult(
                    self._GetUniqueTestName(t),
                    base_test_result.ResultType.TIMEOUT)
                for t in test)
          else:
            results.AddResult(
                base_test_result.BaseTestResult(
                    self._GetUniqueTestName(test),
                    base_test_result.ResultType.TIMEOUT))
        except Exception as e:  # pylint: disable=broad-except
          if isinstance(tests, test_collection.TestCollection):
            rerun = test
          if (isinstance(e, device_errors.DeviceUnreachableError)
              or not isinstance(e, base_error.BaseError)):
            # If we get a device error but believe the device is still
            # reachable, attempt to continue using it. Otherwise, raise
            # the exception and terminate this run_tests_on_device call.
            raise
        finally:
          if isinstance(tests, test_collection.TestCollection):
            if rerun:
              tests.add(rerun)
            tests.test_completed()

      logging.info('Finished running tests on this device.')

    def stop_tests(_signum, _frame):
      logging.critical('Received SIGTERM. Stopping test execution.')
      exit_now.set()
      raise TestsTerminated()

    try:
      with signal_handler.SignalHandler(signal.SIGTERM, stop_tests):
        tries = 0
        results = []
        while tries < self._env.max_tries and tests:
          logging.info('STARTING TRY #%d/%d', tries + 1, self._env.max_tries)
          if tries > 0 and self._env.recover_devices:
            if any(d.build_version_sdk == version_codes.LOLLIPOP_MR1
                   for d in self._env.devices):
              logging.info(
                  'Attempting to recover devices due to known issue on L MR1. '
                  'See crbug.com/787056 for details.')
              self._env.parallel_devices.pMap(
                  device_recovery.RecoverDevice, None)
            elif tries + 1 == self._env.max_tries:
              logging.info(
                  'Attempting to recover devices prior to last test attempt.')
              self._env.parallel_devices.pMap(
                  device_recovery.RecoverDevice, None)
          logging.info('Will run %d tests on %d devices: %s',
                       len(tests), len(self._env.devices),
                       ', '.join(str(d) for d in self._env.devices))
          for t in tests:
            logging.debug('  %s', t)

          try_results = base_test_result.TestRunResults()
          test_names = (self._GetUniqueTestName(t) for t in tests)
          try_results.AddResults(
              base_test_result.BaseTestResult(
                  t, base_test_result.ResultType.NOTRUN)
              for t in test_names if not t.endswith('*'))

          try:
            if self._ShouldShard():
              tc = test_collection.TestCollection(self._CreateShards(tests))
              self._env.parallel_devices.pMap(
                  run_tests_on_device, tc, try_results).pGet(None)
            else:
              self._env.parallel_devices.pMap(
                  run_tests_on_device, tests, try_results).pGet(None)
          except TestsTerminated:
            for unknown_result in try_results.GetUnknown():
              try_results.AddResult(
                  base_test_result.BaseTestResult(
                      unknown_result.GetName(),
                      base_test_result.ResultType.TIMEOUT,
                      log=_SIGTERM_TEST_LOG))
            raise
          finally:
            results.append(try_results)

          tries += 1
          tests = self._GetTestsToRetry(tests, try_results)

          logging.info('FINISHED TRY #%d/%d', tries, self._env.max_tries)
          if tests:
            logging.info('%d failed tests remain.', len(tests))
          else:
            logging.info('All tests completed.')
    except TestsTerminated:
      pass

    return results

  def _GetTestsToRetry(self, tests, try_results):

    def is_failure_result(test_result):
      if isinstance(test_result, list):
        return any(is_failure_result(r) for r in test_result)
      return (
          test_result is None
          or test_result.GetType() not in (
              base_test_result.ResultType.PASS,
              base_test_result.ResultType.SKIP))

    all_test_results = {r.GetName(): r for r in try_results.GetAll()}

    tests_and_names = ((t, self._GetUniqueTestName(t)) for t in tests)

    tests_and_results = {}
    for test, name in tests_and_names:
      if name.endswith('*'):
        tests_and_results[name] = (
            test,
            [r for n, r in all_test_results.iteritems()
             if fnmatch.fnmatch(n, name)])
      else:
        tests_and_results[name] = (test, all_test_results.get(name))

    failed_tests_and_results = (
        (test, result) for test, result in tests_and_results.itervalues()
        if is_failure_result(result)
    )

    return [t for t, r in failed_tests_and_results if self._ShouldRetry(t, r)]

  def _ApplyExternalSharding(self, tests, shard_index, total_shards):
    logging.info('Using external sharding settings. This is shard %d/%d',
                 shard_index, total_shards)

    if total_shards < 0 or shard_index < 0 or total_shards <= shard_index:
      raise InvalidShardingSettings(shard_index, total_shards)

    return [
        t for t in tests
        if hash(self._GetUniqueTestName(t)) % total_shards == shard_index]

  def GetTool(self, device):
    if str(device) not in self._tools:
      self._tools[str(device)] = valgrind_tools.CreateTool(
          self._env.tool, device)
    return self._tools[str(device)]

  def _CreateShards(self, tests):
    raise NotImplementedError

  def _GetUniqueTestName(self, test):
    # pylint: disable=no-self-use
    return test

  def _ShouldRetry(self, test, result):
    # pylint: disable=no-self-use,unused-argument
    return True

  def _GetTests(self):
    raise NotImplementedError

  def _RunTest(self, device, test):
    raise NotImplementedError

  def _ShouldShard(self):
    raise NotImplementedError


class NoTestsError(Exception):
  """Error for when no tests are found."""
