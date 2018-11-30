# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import collections
import io
import json
import logging
import os
import pickle
import shutil
import tempfile
import threading
import time
import zipfile

from devil.android import battery_utils
from devil.android import device_errors
from devil.android import device_list
from devil.android import device_utils
from devil.android import forwarder
from devil.android.tools import device_recovery
from devil.android.tools import device_status
from devil.utils import cmd_helper
from devil.utils import parallelizer
from devil.utils import reraiser_thread
from pylib import constants
from pylib.base import base_test_result
from pylib.constants import host_paths
from pylib.local.device import local_device_environment
from pylib.local.device import local_device_test_run
from py_trace_event import trace_event
from py_utils import contextlib_ext


class HeartBeat(object):

  def __init__(self, shard, wait_time=60*10):
    """ HeartBeat Logger constructor.

    Args:
      shard: A perf test runner device shard.
      wait_time: time to wait between heartbeat messages.
    """
    self._shard = shard
    self._running = False
    self._timer = None
    self._wait_time = wait_time

  def Start(self):
    if not self._running:
      self._timer = threading.Timer(self._wait_time, self._LogMessage)
      self._timer.start()
      self._running = True

  def Stop(self):
    if self._running:
      self._timer.cancel()
      self._running = False

  def _LogMessage(self):
    logging.info('Currently working on test %s', self._shard.current_test)
    self._timer = threading.Timer(self._wait_time, self._LogMessage)
    self._timer.start()


class TestShard(object):
  def __init__(self, env, test_instance, tests, retries=3, timeout=None):
    logging.info('Create shard for the following tests:')
    for t in tests:
      logging.info('  %s', t)
    self._current_test = None
    self._env = env
    self._heart_beat = HeartBeat(self)
    self._index = None
    self._output_dir = None
    self._retries = retries
    self._test_instance = test_instance
    self._tests = tests
    self._timeout = timeout

  def _TestSetUp(self, test):
    if (self._test_instance.collect_chartjson_data
        or self._tests[test].get('archive_output_dir')):
      self._output_dir = tempfile.mkdtemp()

    self._current_test = test
    self._heart_beat.Start()

  def _RunSingleTest(self, test):
    self._test_instance.WriteBuildBotJson(self._output_dir)

    timeout = self._tests[test].get('timeout', self._timeout)
    cmd = self._CreateCmd(test)
    cwd = os.path.abspath(host_paths.DIR_SOURCE_ROOT)

    self._LogTest(test, cmd, timeout)

    try:
      start_time = time.time()

      with contextlib_ext.Optional(
          trace_event.trace(test),
          self._env.trace_output):
        exit_code, output = cmd_helper.GetCmdStatusAndOutputWithTimeout(
            cmd, timeout, cwd=cwd, shell=True)
      end_time = time.time()
      chart_json_output = self._test_instance.ReadChartjsonOutput(
          self._output_dir)
      if exit_code == 0:
        result_type = base_test_result.ResultType.PASS
      else:
        result_type = base_test_result.ResultType.FAIL
    except cmd_helper.TimeoutError as e:
      end_time = time.time()
      exit_code = -1
      output = e.output
      chart_json_output = ''
      result_type = base_test_result.ResultType.TIMEOUT
    return self._ProcessTestResult(test, cmd, start_time, end_time, exit_code,
                                   output, chart_json_output, result_type)

  def _CreateCmd(self, test):
    cmd = []
    if self._test_instance.dry_run:
      cmd.append('echo')
    cmd.append(self._tests[test]['cmd'])
    if self._output_dir:
      cmd.append('--output-dir=%s' % self._output_dir)
    return ' '.join(self._ExtendCmd(cmd))

  def _ExtendCmd(self, cmd): # pylint: disable=no-self-use
    return cmd

  def _LogTest(self, _test, _cmd, _timeout):
    raise NotImplementedError

  def _LogTestExit(self, test, exit_code, duration):
    # pylint: disable=no-self-use
    logging.info('%s : exit_code=%d in %d secs.', test, exit_code, duration)

  def _ExtendPersistedResult(self, persisted_result):
    raise NotImplementedError

  def _ProcessTestResult(self, test, cmd, start_time, end_time, exit_code,
                         output, chart_json_output, result_type):
    if exit_code is None:
      exit_code = -1

    self._LogTestExit(test, exit_code, end_time - start_time)

    archive_bytes = (self._ArchiveOutputDir()
                     if self._tests[test].get('archive_output_dir')
                     else None)
    persisted_result = {
        'name': test,
        'output': [output],
        'chartjson': chart_json_output,
        'archive_bytes': archive_bytes,
        'exit_code': exit_code,
        'result_type': result_type,
        'start_time': start_time,
        'end_time': end_time,
        'total_time': end_time - start_time,
        'cmd': cmd,
    }
    self._ExtendPersistedResult(persisted_result)
    self._SaveResult(persisted_result)
    return result_type

  def _ArchiveOutputDir(self):
    """Archive all files in the output dir, and return as compressed bytes."""
    with io.BytesIO() as archive:
      with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as contents:
        num_files = 0
        for absdir, _, files in os.walk(self._output_dir):
          reldir = os.path.relpath(absdir, self._output_dir)
          for filename in files:
            src_path = os.path.join(absdir, filename)
            # We use normpath to turn './file.txt' into just 'file.txt'.
            dst_path = os.path.normpath(os.path.join(reldir, filename))
            contents.write(src_path, dst_path)
            num_files += 1
      if num_files:
        logging.info('%d files in the output dir were archived.', num_files)
      else:
        logging.warning('No files in the output dir. Archive is empty.')
      return archive.getvalue()

  @staticmethod
  def _SaveResult(result):
    pickled = os.path.join(constants.PERF_OUTPUT_DIR, result['name'])
    if os.path.exists(pickled):
      with file(pickled, 'r') as f:
        previous = pickle.load(f)
        result['output'] = previous['output'] + result['output']
    with file(pickled, 'w') as f:
      pickle.dump(result, f)

  def _TestTearDown(self):
    if self._output_dir:
      shutil.rmtree(self._output_dir, ignore_errors=True)
      self._output_dir = None
    self._heart_beat.Stop()
    self._current_test = None

  @property
  def current_test(self):
    return self._current_test


class DeviceTestShard(TestShard):
  def __init__(
      self, env, test_instance, device, index, tests, retries=3, timeout=None):
    super(DeviceTestShard, self).__init__(
        env, test_instance, tests, retries, timeout)
    self._battery = battery_utils.BatteryUtils(device) if device else None
    self._device = device
    self._index = index

  @local_device_environment.handle_shard_failures
  def RunTestsOnShard(self):
    results = base_test_result.TestRunResults()
    for test in self._tests:
      tries_left = self._retries
      result_type = None
      while (result_type != base_test_result.ResultType.PASS
             and tries_left > 0):
        try:
          self._TestSetUp(test)
          result_type = self._RunSingleTest(test)
        except device_errors.CommandTimeoutError:
          result_type = base_test_result.ResultType.TIMEOUT
        except (device_errors.CommandFailedError,
                device_errors.DeviceUnreachableError):
          logging.exception('Exception when executing %s.', test)
          result_type = base_test_result.ResultType.FAIL
        finally:
          self._TestTearDown()
          if result_type != base_test_result.ResultType.PASS:
            try:
              device_recovery.RecoverDevice(self._device, self._env.blacklist)
            except device_errors.CommandTimeoutError:
              logging.exception(
                  'Device failed to recover after failing %s.', test)
          tries_left -= 1

      results.AddResult(base_test_result.BaseTestResult(test, result_type))
    return results

  def _LogTestExit(self, test, exit_code, duration):
    logging.info('%s : exit_code=%d in %d secs on device %s',
                 test, exit_code, duration, str(self._device))

  @trace_event.traced
  def _TestSetUp(self, test):
    if not self._device.IsOnline():
      msg = 'Device %s is unresponsive.' % str(self._device)
      raise device_errors.DeviceUnreachableError(msg)

    logging.info('Charge level: %s%%',
                 str(self._battery.GetBatteryInfo().get('level')))
    if self._test_instance.min_battery_level:
      self._battery.ChargeDeviceToLevel(self._test_instance.min_battery_level)

    logging.info('temperature: %s (0.1 C)',
                 str(self._battery.GetBatteryInfo().get('temperature')))
    if self._test_instance.max_battery_temp:
      self._battery.LetBatteryCoolToTemperature(
          self._test_instance.max_battery_temp)

    if not self._device.IsScreenOn():
      self._device.SetScreen(True)

    super(DeviceTestShard, self)._TestSetUp(test)

  def _LogTest(self, test, cmd, timeout):
    logging.debug("Running %s with command '%s' on shard %s with timeout %d",
                  test, cmd, str(self._index), timeout)

  def _ExtendCmd(self, cmd):
    cmd.extend(['--device=%s' % str(self._device)])
    return cmd

  def _ExtendPersistedResult(self, persisted_result):
    persisted_result['host_test'] = False
    persisted_result['device'] = str(self._device)

  @trace_event.traced
  def _TestTearDown(self):
    try:
      logging.info('Unmapping device ports for %s.', self._device)
      forwarder.Forwarder.UnmapAllDevicePorts(self._device)
    except Exception: # pylint: disable=broad-except
      logging.exception('Exception when resetting ports.')
    finally:
      super(DeviceTestShard, self)._TestTearDown()

class HostTestShard(TestShard):
  def __init__(self, env, test_instance, tests, retries=3, timeout=None):
    super(HostTestShard, self).__init__(
        env, test_instance, tests, retries, timeout)

  @local_device_environment.handle_shard_failures
  def RunTestsOnShard(self):
    results = base_test_result.TestRunResults()
    for test in self._tests:
      tries_left = self._retries + 1
      result_type = None
      while (result_type != base_test_result.ResultType.PASS
             and tries_left > 0):
        try:
          self._TestSetUp(test)
          result_type = self._RunSingleTest(test)
        finally:
          self._TestTearDown()
          tries_left -= 1
      results.AddResult(base_test_result.BaseTestResult(test, result_type))
    return results

  def _LogTest(self, test, cmd, timeout):
    logging.debug("Running %s with command '%s' on host shard with timeout %d",
                  test, cmd, timeout)

  def _ExtendPersistedResult(self, persisted_result):
    persisted_result['host_test'] = True


class LocalDevicePerfTestRun(local_device_test_run.LocalDeviceTestRun):

  _DEFAULT_TIMEOUT = 5 * 60 * 60  # 5 hours.
  _CONFIG_VERSION = 1

  def __init__(self, env, test_instance):
    super(LocalDevicePerfTestRun, self).__init__(env, test_instance)
    self._devices = None
    self._env = env
    self._no_device_tests = {}
    self._test_buckets = []
    self._test_instance = test_instance
    self._timeout = None if test_instance.no_timeout else self._DEFAULT_TIMEOUT

  #override
  def SetUp(self):
    if os.path.exists(constants.PERF_OUTPUT_DIR):
      shutil.rmtree(constants.PERF_OUTPUT_DIR)
    os.makedirs(constants.PERF_OUTPUT_DIR)

  #override
  def TearDown(self):
    pass

  def _GetStepsFromDict(self):
    # From where this is called one of these two must be set.
    if self._test_instance.single_step:
      return {
          'version': self._CONFIG_VERSION,
          'steps': {
              'single_step': {
                'device_affinity': 0,
                'cmd': self._test_instance.single_step
              },
          }
      }
    if self._test_instance.steps:
      with file(self._test_instance.steps, 'r') as f:
        steps = json.load(f)
        if steps['version'] != self._CONFIG_VERSION:
          raise TestDictVersionError(
              'Version is expected to be %d but was %d' % (self._CONFIG_VERSION,
                                                           steps['version']))
        return steps
    raise PerfTestRunGetStepsError(
        'Neither single_step or steps set in test_instance.')

  def _SplitTestsByAffinity(self):
    # This splits tests by their device affinity so that the same tests always
    # run on the same devices. This is important for perf tests since different
    # devices might yield slightly different performance results.
    test_dict = self._GetStepsFromDict()
    for test, test_config in sorted(test_dict['steps'].iteritems()):
      try:
        affinity = test_config.get('device_affinity')
        if affinity is None:
          self._no_device_tests[test] = test_config
        else:
          if len(self._test_buckets) < affinity + 1:
            while len(self._test_buckets) != affinity + 1:
              self._test_buckets.append(collections.OrderedDict())
          self._test_buckets[affinity][test] = test_config
      except KeyError:
        logging.exception(
            'Test config for %s is bad.\n Config:%s', test, str(test_config))

  @staticmethod
  def _GetAllDevices(active_devices, devices_path):
    try:
      if devices_path:
        devices = [device_utils.DeviceUtils(s)
                   for s in device_list.GetPersistentDeviceList(devices_path)]
        if not devices and active_devices:
          logging.warning('%s is empty. Falling back to active devices.',
                          devices_path)
          devices = active_devices
      else:
        logging.warning('Known devices file path not being passed. For device '
                        'affinity to work properly, it must be passed.')
        devices = active_devices
    except IOError as e:
      logging.error('Unable to find %s [%s]', devices_path, e)
      devices = active_devices
    return sorted(devices)

  #override
  def RunTests(self):
    def run_no_devices_tests():
      if not self._no_device_tests:
        return []
      s = HostTestShard(self._env, self._test_instance, self._no_device_tests,
                        retries=3, timeout=self._timeout)
      return [s.RunTestsOnShard()]

    def device_shard_helper(shard_id):
      if device_status.IsBlacklisted(
           str(self._devices[shard_id]), self._env.blacklist):
        logging.warning('Device %s is not active. Will not create shard %s.',
                        str(self._devices[shard_id]), shard_id)
        return None
      s = DeviceTestShard(self._env, self._test_instance,
                          self._devices[shard_id], shard_id,
                          self._test_buckets[shard_id],
                          retries=self._env.max_tries, timeout=self._timeout)
      return s.RunTestsOnShard()

    def run_devices_tests():
      if not self._test_buckets:
        return []
      if self._devices is None:
        self._devices = self._GetAllDevices(
            self._env.devices, self._test_instance.known_devices_file)

      device_indices = range(min(len(self._devices), len(self._test_buckets)))
      shards = parallelizer.Parallelizer(device_indices).pMap(
          device_shard_helper)
      return [x for x in shards.pGet(self._timeout) if x is not None]

    # Affinitize the tests.
    self._SplitTestsByAffinity()
    if not self._test_buckets and not self._no_device_tests:
      raise local_device_test_run.NoTestsError()
    host_test_results, device_test_results = reraiser_thread.RunAsync(
        [run_no_devices_tests, run_devices_tests])

    return host_test_results + device_test_results

  # override
  def TestPackage(self):
    return 'perf'

  # override
  def _CreateShards(self, _tests):
    raise NotImplementedError

  # override
  def _GetTests(self):
    return self._test_buckets

  # override
  def _RunTest(self, _device, _test):
    raise NotImplementedError

  # override
  def _ShouldShard(self):
    return False


class OutputJsonList(LocalDevicePerfTestRun):
  # override
  def SetUp(self):
    pass

  # override
  def RunTests(self):
    result_type = self._test_instance.OutputJsonList()
    result = base_test_result.TestRunResults()
    result.AddResult(
        base_test_result.BaseTestResult('OutputJsonList', result_type))
    return [result]

  # override
  def _CreateShards(self, _tests):
    raise NotImplementedError

  # override
  def _RunTest(self, _device, _test):
    raise NotImplementedError


class PrintStep(LocalDevicePerfTestRun):
  # override
  def SetUp(self):
    pass

  # override
  def RunTests(self):
    result_type = self._test_instance.PrintTestOutput()
    result = base_test_result.TestRunResults()
    result.AddResult(
        base_test_result.BaseTestResult('PrintStep', result_type))
    return [result]

  # override
  def _CreateShards(self, _tests):
    raise NotImplementedError

  # override
  def _RunTest(self, _device, _test):
    raise NotImplementedError


class TestDictVersionError(Exception):
  pass

class PerfTestRunGetStepsError(Exception):
  pass
