# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging

from devil.android import device_errors
from devil.android.sdk import intent
from pylib import constants
from pylib.base import base_test_result
from pylib.local.device import local_device_test_run


_CHROME_PACKAGE = constants.PACKAGE_INFO['chrome'].package

class LocalDeviceMonkeyTestRun(local_device_test_run.LocalDeviceTestRun):
  def __init__(self, env, test_instance):
    super(LocalDeviceMonkeyTestRun, self).__init__(env, test_instance)

  def TestPackage(self):
    return 'monkey'

  #override
  def SetUp(self):
    pass

  #override
  def _RunTest(self, device, test):
    device.ClearApplicationState(self._test_instance.package)

    # Chrome crashes are not always caught by Monkey test runner.
    # Launch Chrome and verify Chrome has the same PID before and after
    # the test.
    device.StartActivity(
        intent.Intent(package=self._test_instance.package,
                      activity=self._test_instance.activity,
                      action='android.intent.action.MAIN'),
        blocking=True, force_stop=True)
    before_pids = device.GetPids(self._test_instance.package)

    output = ''
    if before_pids:
      if len(before_pids.get(self._test_instance.package, [])) > 1:
        raise Exception(
            'At most one instance of process %s expected but found pids: '
            '%s' % (self._test_instance.package, before_pids))
      output = '\n'.join(self._LaunchMonkeyTest(device))
      after_pids = device.GetPids(self._test_instance.package)

    crashed = True
    if not self._test_instance.package in before_pids:
      logging.error('Failed to start the process.')
    elif not self._test_instance.package in after_pids:
      logging.error('Process %s has died.',
                    before_pids[self._test_instance.package])
    elif (before_pids[self._test_instance.package] !=
          after_pids[self._test_instance.package]):
      logging.error('Detected process restart %s -> %s',
                    before_pids[self._test_instance.package],
                    after_pids[self._test_instance.package])
    else:
      crashed = False

    success_pattern = 'Events injected: %d' % self._test_instance.event_count
    if success_pattern in output and not crashed:
      result = base_test_result.BaseTestResult(
          test, base_test_result.ResultType.PASS, log=output)
    else:
      result = base_test_result.BaseTestResult(
          test, base_test_result.ResultType.FAIL, log=output)
      if 'chrome' in self._test_instance.package:
        logging.warning('Starting MinidumpUploadService...')
        # TODO(jbudorick): Update this after upstreaming.
        minidump_intent = intent.Intent(
            action='%s.crash.ACTION_FIND_ALL' % _CHROME_PACKAGE,
            package=self._test_instance.package,
            activity='%s.crash.MinidumpUploadService' % _CHROME_PACKAGE)
        try:
          device.RunShellCommand(
              ['am', 'startservice'] + minidump_intent.am_args,
              as_root=True, check_return=True)
        except device_errors.CommandFailedError:
          logging.exception('Failed to start MinidumpUploadService')

    return result, None

  #override
  def TearDown(self):
    pass

  #override
  def _CreateShards(self, tests):
    return tests

  #override
  def _ShouldShard(self):
    # TODO(mikecase): Run Monkey test concurrently on each attached device.
    return False

  #override
  def _GetTests(self):
    return ['MonkeyTest']

  def _LaunchMonkeyTest(self, device):
    try:
      cmd = ['monkey',
             '-p', self._test_instance.package,
             '--throttle', str(self._test_instance.throttle),
             '-s', str(self._test_instance.seed),
             '--monitor-native-crashes',
             '--kill-process-after-error']
      for category in self._test_instance.categories:
        cmd.extend(['-c', category])
      for _ in range(self._test_instance.verbose_count):
        cmd.append('-v')
      cmd.append(str(self._test_instance.event_count))
      return device.RunShellCommand(
          cmd, timeout=self._test_instance.timeout, check_return=True)
    finally:
      try:
        # Kill the monkey test process on the device. If you manually
        # interrupt the test run, this will prevent the monkey test from
        # continuing to run.
        device.KillAll('com.android.commands.monkey')
      except device_errors.CommandFailedError:
        pass
