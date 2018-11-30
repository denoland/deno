# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import collections
import itertools
import logging
import os
import posixpath
import shutil
import time

from devil.android import crash_handler
from devil.android import device_errors
from devil.android import device_temp_file
from devil.android import logcat_monitor
from devil.android import ports
from devil.utils import reraiser_thread
from incremental_install import installer
from pylib import constants
from pylib.base import base_test_result
from pylib.gtest import gtest_test_instance
from pylib.local import local_test_server_spawner
from pylib.local.device import local_device_environment
from pylib.local.device import local_device_test_run
from pylib.utils import google_storage_helper
from pylib.utils import logdog_helper
from py_trace_event import trace_event
from py_utils import contextlib_ext
from py_utils import tempfile_ext
import tombstones

_MAX_INLINE_FLAGS_LENGTH = 50  # Arbitrarily chosen.
_EXTRA_COMMAND_LINE_FILE = (
    'org.chromium.native_test.NativeTest.CommandLineFile')
_EXTRA_COMMAND_LINE_FLAGS = (
    'org.chromium.native_test.NativeTest.CommandLineFlags')
_EXTRA_STDOUT_FILE = (
    'org.chromium.native_test.NativeTestInstrumentationTestRunner'
        '.StdoutFile')
_EXTRA_TEST = (
    'org.chromium.native_test.NativeTestInstrumentationTestRunner'
        '.Test')
_EXTRA_TEST_LIST = (
    'org.chromium.native_test.NativeTestInstrumentationTestRunner'
        '.TestList')

_MAX_SHARD_SIZE = 256
_SECONDS_TO_NANOS = int(1e9)

# The amount of time a test executable may run before it gets killed.
_TEST_TIMEOUT_SECONDS = 30*60

# Tests that use SpawnedTestServer must run the LocalTestServerSpawner on the
# host machine.
# TODO(jbudorick): Move this up to the test instance if the net test server is
# handled outside of the APK for the remote_device environment.
_SUITE_REQUIRES_TEST_SERVER_SPAWNER = [
  'components_browsertests', 'content_unittests', 'content_browsertests',
  'net_unittests', 'services_unittests', 'unit_tests'
]

# No-op context manager. If we used Python 3, we could change this to
# contextlib.ExitStack()
class _NullContextManager(object):
  def __enter__(self):
    pass
  def __exit__(self, *args):
    pass


# TODO(jbudorick): Move this inside _ApkDelegate once TestPackageApk is gone.
def PullAppFilesImpl(device, package, files, directory):
  device_dir = device.GetApplicationDataDirectory(package)
  host_dir = os.path.join(directory, str(device))
  for f in files:
    device_file = posixpath.join(device_dir, f)
    host_file = os.path.join(host_dir, *f.split(posixpath.sep))
    host_file_base, ext = os.path.splitext(host_file)
    for i in itertools.count():
      host_file = '%s_%d%s' % (host_file_base, i, ext)
      if not os.path.exists(host_file):
        break
    device.PullFile(device_file, host_file)


def _ExtractTestsFromFilter(gtest_filter):
  """Returns the list of tests specified by the given filter.

  Returns:
    None if the device should be queried for the test list instead.
  """
  # Empty means all tests, - means exclude filter.
  if not gtest_filter or '-' in gtest_filter:
    return None

  patterns = gtest_filter.split(':')
  # For a single pattern, allow it even if it has a wildcard so long as the
  # wildcard comes at the end and there is at least one . to prove the scope is
  # not too large.
  # This heuristic is not necessarily faster, but normally is.
  if len(patterns) == 1 and patterns[0].endswith('*'):
    no_suffix = patterns[0].rstrip('*')
    if '*' not in no_suffix and '.' in no_suffix:
      return patterns

  if '*' in gtest_filter:
    return None
  return patterns


class _ApkDelegate(object):
  def __init__(self, test_instance, tool):
    self._activity = test_instance.activity
    self._apk_helper = test_instance.apk_helper
    self._test_apk_incremental_install_json = (
        test_instance.test_apk_incremental_install_json)
    self._package = test_instance.package
    self._runner = test_instance.runner
    self._permissions = test_instance.permissions
    self._suite = test_instance.suite
    self._component = '%s/%s' % (self._package, self._runner)
    self._extras = test_instance.extras
    self._wait_for_java_debugger = test_instance.wait_for_java_debugger
    self._tool = tool

  def GetTestDataRoot(self, device):
    # pylint: disable=no-self-use
    return posixpath.join(device.GetExternalStoragePath(),
                          'chromium_tests_root')

  def Install(self, device):
    if self._test_apk_incremental_install_json:
      installer.Install(device, self._test_apk_incremental_install_json,
                        apk=self._apk_helper, permissions=self._permissions)
    else:
      device.Install(self._apk_helper, reinstall=True,
                     permissions=self._permissions)

  def ResultsDirectory(self, device):
    return device.GetApplicationDataDirectory(self._package)

  def Run(self, test, device, flags=None, **kwargs):
    extras = dict(self._extras)

    if ('timeout' in kwargs
        and gtest_test_instance.EXTRA_SHARD_NANO_TIMEOUT not in extras):
      # Make sure the instrumentation doesn't kill the test before the
      # scripts do. The provided timeout value is in seconds, but the
      # instrumentation deals with nanoseconds because that's how Android
      # handles time.
      extras[gtest_test_instance.EXTRA_SHARD_NANO_TIMEOUT] = int(
          kwargs['timeout'] * _SECONDS_TO_NANOS)

    # pylint: disable=redefined-variable-type
    command_line_file = _NullContextManager()
    if flags:
      if len(flags) > _MAX_INLINE_FLAGS_LENGTH:
        command_line_file = device_temp_file.DeviceTempFile(device.adb)
        device.WriteFile(command_line_file.name, '_ %s' % flags)
        extras[_EXTRA_COMMAND_LINE_FILE] = command_line_file.name
      else:
        extras[_EXTRA_COMMAND_LINE_FLAGS] = flags

    test_list_file = _NullContextManager()
    if test:
      if len(test) > 1:
        test_list_file = device_temp_file.DeviceTempFile(device.adb)
        device.WriteFile(test_list_file.name, '\n'.join(test))
        extras[_EXTRA_TEST_LIST] = test_list_file.name
      else:
        extras[_EXTRA_TEST] = test[0]
    # pylint: enable=redefined-variable-type

    stdout_file = device_temp_file.DeviceTempFile(
        device.adb, dir=device.GetExternalStoragePath(), suffix='.gtest_out')
    extras[_EXTRA_STDOUT_FILE] = stdout_file.name

    if self._wait_for_java_debugger:
      cmd = ['am', 'set-debug-app', '-w', self._package]
      device.RunShellCommand(cmd, check_return=True)
      logging.warning('*' * 80)
      logging.warning('Waiting for debugger to attach to process: %s',
                      self._package)
      logging.warning('*' * 80)

    with command_line_file, test_list_file, stdout_file:
      try:
        device.StartInstrumentation(
            self._component, extras=extras, raw=False, **kwargs)
      except device_errors.CommandFailedError:
        logging.exception('gtest shard failed.')
      except device_errors.CommandTimeoutError:
        logging.exception('gtest shard timed out.')
      except device_errors.DeviceUnreachableError:
        logging.exception('gtest shard device unreachable.')
      except Exception:
        device.ForceStop(self._package)
        raise
      # TODO(jbudorick): Remove this after resolving crbug.com/726880
      logging.info(
          '%s size on device: %s',
          stdout_file.name, device.StatPath(stdout_file.name).get('st_size', 0))
      return device.ReadFile(stdout_file.name).splitlines()

  def PullAppFiles(self, device, files, directory):
    PullAppFilesImpl(device, self._package, files, directory)

  def Clear(self, device):
    device.ClearApplicationState(self._package, permissions=self._permissions)


class _ExeDelegate(object):
  def __init__(self, tr, dist_dir, tool):
    self._host_dist_dir = dist_dir
    self._exe_file_name = os.path.basename(dist_dir)[:-len('__dist')]
    self._device_dist_dir = posixpath.join(
        constants.TEST_EXECUTABLE_DIR, os.path.basename(dist_dir))
    self._test_run = tr
    self._tool = tool

  def GetTestDataRoot(self, device):
    # pylint: disable=no-self-use
    # pylint: disable=unused-argument
    return posixpath.join(constants.TEST_EXECUTABLE_DIR, 'chromium_tests_root')

  def Install(self, device):
    # TODO(jbudorick): Look into merging this with normal data deps pushing if
    # executables become supported on nonlocal environments.
    device.PushChangedFiles([(self._host_dist_dir, self._device_dist_dir)],
                            delete_device_stale=True)

  def ResultsDirectory(self, device):
    # pylint: disable=no-self-use
    # pylint: disable=unused-argument
    return constants.TEST_EXECUTABLE_DIR

  def Run(self, test, device, flags=None, **kwargs):
    tool = self._test_run.GetTool(device).GetTestWrapper()
    if tool:
      cmd = [tool]
    else:
      cmd = []
    cmd.append(posixpath.join(self._device_dist_dir, self._exe_file_name))

    if test:
      cmd.append('--gtest_filter=%s' % ':'.join(test))
    if flags:
      # TODO(agrieve): This won't work if multiple flags are passed.
      cmd.append(flags)
    cwd = constants.TEST_EXECUTABLE_DIR

    env = {
      'LD_LIBRARY_PATH': self._device_dist_dir
    }

    if self._tool != 'asan':
      env['UBSAN_OPTIONS'] = constants.UBSAN_OPTIONS

    try:
      gcov_strip_depth = os.environ['NATIVE_COVERAGE_DEPTH_STRIP']
      external = device.GetExternalStoragePath()
      env['GCOV_PREFIX'] = '%s/gcov' % external
      env['GCOV_PREFIX_STRIP'] = gcov_strip_depth
    except (device_errors.CommandFailedError, KeyError):
      pass

    # Executable tests return a nonzero exit code on test failure, which is
    # fine from the test runner's perspective; thus check_return=False.
    output = device.RunShellCommand(
        cmd, cwd=cwd, env=env, check_return=False, large_output=True, **kwargs)
    return output

  def PullAppFiles(self, device, files, directory):
    pass

  def Clear(self, device):
    device.KillAll(self._exe_file_name, blocking=True, timeout=30, quiet=True)


class LocalDeviceGtestRun(local_device_test_run.LocalDeviceTestRun):

  def __init__(self, env, test_instance):
    assert isinstance(env, local_device_environment.LocalDeviceEnvironment)
    assert isinstance(test_instance, gtest_test_instance.GtestTestInstance)
    super(LocalDeviceGtestRun, self).__init__(env, test_instance)

    # pylint: disable=redefined-variable-type
    if self._test_instance.apk:
      self._delegate = _ApkDelegate(self._test_instance, env.tool)
    elif self._test_instance.exe_dist_dir:
      self._delegate = _ExeDelegate(self, self._test_instance.exe_dist_dir,
                                    self._env.tool)
    # pylint: enable=redefined-variable-type
    self._crashes = set()
    self._servers = collections.defaultdict(list)

  #override
  def TestPackage(self):
    return self._test_instance.suite

  #override
  def SetUp(self):
    @local_device_environment.handle_shard_failures_with(
        on_failure=self._env.BlacklistDevice)
    @trace_event.traced
    def individual_device_set_up(device, host_device_tuples):
      def install_apk(dev):
        # Install test APK.
        self._delegate.Install(dev)

      def push_test_data(dev):
        # Push data dependencies.
        device_root = self._delegate.GetTestDataRoot(dev)
        host_device_tuples_substituted = [
            (h, local_device_test_run.SubstituteDeviceRoot(d, device_root))
            for h, d in host_device_tuples]
        dev.PushChangedFiles(
            host_device_tuples_substituted,
            delete_device_stale=True)
        if not host_device_tuples:
          dev.RemovePath(device_root, force=True, recursive=True, rename=True)
          dev.RunShellCommand(['mkdir', '-p', device_root], check_return=True)

      def init_tool_and_start_servers(dev):
        tool = self.GetTool(dev)
        tool.CopyFiles(dev)
        tool.SetupEnvironment()

        self._servers[str(dev)] = []
        if self.TestPackage() in _SUITE_REQUIRES_TEST_SERVER_SPAWNER:
          self._servers[str(dev)].append(
              local_test_server_spawner.LocalTestServerSpawner(
                  ports.AllocateTestServerPort(), dev, tool))

        for s in self._servers[str(dev)]:
          s.SetUp()

      def bind_crash_handler(step, dev):
        return lambda: crash_handler.RetryOnSystemCrash(step, dev)

      steps = [
          bind_crash_handler(s, device)
          for s in (install_apk, push_test_data, init_tool_and_start_servers)]
      if self._env.concurrent_adb:
        reraiser_thread.RunAsync(steps)
      else:
        for step in steps:
          step()

    self._env.parallel_devices.pMap(
        individual_device_set_up,
        self._test_instance.GetDataDependencies())

  #override
  def _ShouldShard(self):
    return True

  #override
  def _CreateShards(self, tests):
    # _crashes are tests that might crash and make the tests in the same shard
    # following the crashed testcase not run.
    # Thus we need to create separate shards for each crashed testcase,
    # so that other tests can be run.
    device_count = len(self._env.devices)
    shards = []

    # Add shards with only one suspect testcase.
    shards += [[crash] for crash in self._crashes if crash in tests]

    # Delete suspect testcase from tests.
    tests = [test for test in tests if not test in self._crashes]

    for i in xrange(0, device_count):
      unbounded_shard = tests[i::device_count]
      shards += [unbounded_shard[j:j+_MAX_SHARD_SIZE]
                 for j in xrange(0, len(unbounded_shard), _MAX_SHARD_SIZE)]
    return shards

  #override
  def _GetTests(self):
    if self._test_instance.extract_test_list_from_filter:
      # When the exact list of tests to run is given via command-line (e.g. when
      # locally iterating on a specific test), skip querying the device (which
      # takes ~3 seconds).
      tests = _ExtractTestsFromFilter(self._test_instance.gtest_filter)
      if tests:
        return tests

    # Even when there's only one device, it still makes sense to retrieve the
    # test list so that tests can be split up and run in batches rather than all
    # at once (since test output is not streamed).
    @local_device_environment.handle_shard_failures_with(
        on_failure=self._env.BlacklistDevice)
    def list_tests(dev):
      timeout = 30
      retries = 1
      if self._test_instance.wait_for_java_debugger:
        timeout = None

      flags = list(self._test_instance.flags)
      flags.append('--gtest_list_tests')

      # TODO(crbug.com/726880): Remove retries when no longer necessary.
      for i in range(0, retries+1):
        logging.info('flags:')
        for f in flags:
          logging.info('  %s', f)

        raw_test_list = crash_handler.RetryOnSystemCrash(
            lambda d: self._delegate.Run(
                None, d, flags=' '.join(flags), timeout=timeout),
            device=dev)
        tests = gtest_test_instance.ParseGTestListTests(raw_test_list)
        if not tests:
          logging.info('No tests found. Output:')
          for l in raw_test_list:
            logging.info('  %s', l)
          logging.info('Logcat:')
          for line in dev.adb.Logcat(dump=True):
            logging.info(line)
          dev.adb.Logcat(clear=True)
          if i < retries:
            logging.info('Retrying...')
        else:
          break
      return tests

    # Query all devices in case one fails.
    test_lists = self._env.parallel_devices.pMap(list_tests).pGet(None)

    # If all devices failed to list tests, raise an exception.
    # Check that tl is not None and is not empty.
    if all(not tl for tl in test_lists):
      raise device_errors.CommandFailedError(
          'Failed to list tests on any device')
    tests = list(sorted(set().union(*[set(tl) for tl in test_lists if tl])))
    tests = self._test_instance.FilterTests(tests)
    tests = self._ApplyExternalSharding(
        tests, self._test_instance.external_shard_index,
        self._test_instance.total_external_shards)
    return tests

  def _UploadTestArtifacts(self, device, test_artifacts_dir):
    # TODO(jbudorick): Reconcile this with the output manager once
    # https://codereview.chromium.org/2933993002/ lands.
    if test_artifacts_dir:
      with tempfile_ext.NamedTemporaryDirectory() as test_artifacts_host_dir:
        device.PullFile(test_artifacts_dir.name, test_artifacts_host_dir)
        with tempfile_ext.NamedTemporaryDirectory() as temp_zip_dir:
          zip_base_name = os.path.join(temp_zip_dir, 'test_artifacts')
          test_artifacts_zip = shutil.make_archive(
              zip_base_name, 'zip', test_artifacts_host_dir)
          link = google_storage_helper.upload(
              google_storage_helper.unique_name(
                  'test_artifacts', device=device),
              test_artifacts_zip,
              bucket='%s/test_artifacts' % (
                  self._test_instance.gs_test_artifacts_bucket))
          logging.info('Uploading test artifacts to %s.', link)
          return link
    return None

  #override
  def _RunTest(self, device, test):
    # Run the test.
    timeout = (self._test_instance.shard_timeout
               * self.GetTool(device).GetTimeoutScale())
    if self._test_instance.wait_for_java_debugger:
      timeout = None
    if self._test_instance.store_tombstones:
      tombstones.ClearAllTombstones(device)
    with device_temp_file.DeviceTempFile(
        adb=device.adb,
        dir=self._delegate.ResultsDirectory(device),
        suffix='.xml') as device_tmp_results_file:
      with contextlib_ext.Optional(
          device_temp_file.NamedDeviceTemporaryDirectory(
              adb=device.adb, dir='/sdcard/'),
          self._test_instance.gs_test_artifacts_bucket) as test_artifacts_dir:
        with (contextlib_ext.Optional(
            device_temp_file.DeviceTempFile(
                adb=device.adb, dir=self._delegate.ResultsDirectory(device)),
            self._test_instance.isolated_script_test_perf_output)
            ) as isolated_script_test_perf_output:

          flags = list(self._test_instance.flags)
          if self._test_instance.enable_xml_result_parsing:
            flags.append('--gtest_output=xml:%s' % device_tmp_results_file.name)

          if self._test_instance.gs_test_artifacts_bucket:
            flags.append('--test_artifacts_dir=%s' % test_artifacts_dir.name)

          if self._test_instance.isolated_script_test_perf_output:
            flags.append('--isolated_script_test_perf_output=%s'
                         % isolated_script_test_perf_output.name)

          logging.info('flags:')
          for f in flags:
            logging.info('  %s', f)

          stream_name = 'logcat_%s_%s_%s' % (
              hash(tuple(test)),
              time.strftime('%Y%m%dT%H%M%S-UTC', time.gmtime()),
              device.serial)

          with self._env.output_manager.ArchivedTempfile(
              stream_name, 'logcat') as logcat_file:
            with logcat_monitor.LogcatMonitor(
                device.adb,
                filter_specs=local_device_environment.LOGCAT_FILTERS,
                output_file=logcat_file.name) as logmon:
              with contextlib_ext.Optional(
                  trace_event.trace(str(test)),
                  self._env.trace_output):
                output = self._delegate.Run(
                    test, device, flags=' '.join(flags),
                    timeout=timeout, retries=0)
            logmon.Close()

          if logcat_file.Link():
            logging.info('Logcat saved to %s', logcat_file.Link())

          if self._test_instance.enable_xml_result_parsing:
            try:
              gtest_xml = device.ReadFile(
                  device_tmp_results_file.name,
                  as_root=True)
            except device_errors.CommandFailedError as e:
              logging.warning(
                  'Failed to pull gtest results XML file %s: %s',
                  device_tmp_results_file.name,
                  str(e))
              gtest_xml = None

          if self._test_instance.isolated_script_test_perf_output:
            try:
              device.PullFile(
                  isolated_script_test_perf_output.name,
                  self._test_instance.isolated_script_test_perf_output)
            except device_errors.CommandFailedError as e:
              logging.warning(
                  'Failed to pull chartjson results %s: %s',
                  isolated_script_test_perf_output.name, str(e))

          test_artifacts_url = self._UploadTestArtifacts(device,
                                                         test_artifacts_dir)

    for s in self._servers[str(device)]:
      s.Reset()
    if self._test_instance.app_files:
      self._delegate.PullAppFiles(device, self._test_instance.app_files,
                                  self._test_instance.app_file_dir)
    if not self._env.skip_clear_data:
      self._delegate.Clear(device)

    for l in output:
      logging.info(l)

    # Parse the output.
    # TODO(jbudorick): Transition test scripts away from parsing stdout.
    if self._test_instance.enable_xml_result_parsing:
      results = gtest_test_instance.ParseGTestXML(gtest_xml)
    else:
      results = gtest_test_instance.ParseGTestOutput(
          output, self._test_instance.symbolizer, device.product_cpu_abi)

    tombstones_url = None
    for r in results:
      if logcat_file:
        r.SetLink('logcat', logcat_file.Link())

      if self._test_instance.gs_test_artifacts_bucket:
        r.SetLink('test_artifacts', test_artifacts_url)

      if r.GetType() == base_test_result.ResultType.CRASH:
        self._crashes.add(r.GetName())
        if self._test_instance.store_tombstones:
          if not tombstones_url:
            resolved_tombstones = tombstones.ResolveTombstones(
                device,
                resolve_all_tombstones=True,
                include_stack_symbols=False,
                wipe_tombstones=True)
            stream_name = 'tombstones_%s_%s' % (
                time.strftime('%Y%m%dT%H%M%S', time.localtime()),
                device.serial)
            tombstones_url = logdog_helper.text(
                stream_name, '\n'.join(resolved_tombstones))
          r.SetLink('tombstones', tombstones_url)

    tests_stripped_disabled_prefix = set()
    for t in test:
      tests_stripped_disabled_prefix.add(
          gtest_test_instance.TestNameWithoutDisabledPrefix(t))
    not_run_tests = tests_stripped_disabled_prefix.difference(
        set(r.GetName() for r in results))
    return results, list(not_run_tests) if results else None

  #override
  def TearDown(self):
    @local_device_environment.handle_shard_failures
    @trace_event.traced
    def individual_device_tear_down(dev):
      for s in self._servers.get(str(dev), []):
        s.TearDown()

      tool = self.GetTool(dev)
      tool.CleanUpEnvironment()

    self._env.parallel_devices.pMap(individual_device_tear_down)
