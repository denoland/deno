# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import contextlib
import copy
import hashlib
import json
import logging
import os
import posixpath
import re
import sys
import time

from devil.android import crash_handler
from devil.android import device_errors
from devil.android import device_temp_file
from devil.android import flag_changer
from devil.android.sdk import shared_prefs
from devil.android import logcat_monitor
from devil.android.tools import system_app
from devil.utils import reraiser_thread
from incremental_install import installer
from pylib import constants
from pylib import valgrind_tools
from pylib.base import base_test_result
from pylib.base import output_manager
from pylib.constants import host_paths
from pylib.instrumentation import instrumentation_test_instance
from pylib.local.device import local_device_environment
from pylib.local.device import local_device_test_run
from pylib.output import remote_output_manager
from pylib.utils import instrumentation_tracing
from pylib.utils import shared_preference_utils

from py_trace_event import trace_event
from py_trace_event import trace_time
from py_utils import contextlib_ext
from py_utils import tempfile_ext
import tombstones


with host_paths.SysPath(
    os.path.join(host_paths.DIR_SOURCE_ROOT, 'third_party'), 0):
  import jinja2  # pylint: disable=import-error
  import markupsafe  # pylint: disable=import-error,unused-import


_JINJA_TEMPLATE_DIR = os.path.join(
    host_paths.DIR_SOURCE_ROOT, 'build', 'android', 'pylib', 'instrumentation')
_JINJA_TEMPLATE_FILENAME = 'render_test.html.jinja'

_TAG = 'test_runner_py'

TIMEOUT_ANNOTATIONS = [
  ('Manual', 10 * 60 * 60),
  ('IntegrationTest', 30 * 60),
  ('External', 10 * 60),
  ('EnormousTest', 10 * 60),
  ('LargeTest', 5 * 60),
  ('MediumTest', 3 * 60),
  ('SmallTest', 1 * 60),
]

LOGCAT_FILTERS = ['*:e', 'chromium:v', 'cr_*:v', 'DEBUG:I',
                  'StrictMode:D', '%s:I' % _TAG]

EXTRA_SCREENSHOT_FILE = (
    'org.chromium.base.test.ScreenshotOnFailureStatement.ScreenshotFile')

EXTRA_UI_CAPTURE_DIR = (
    'org.chromium.base.test.util.Screenshooter.ScreenshotDir')

EXTRA_TRACE_FILE = ('org.chromium.base.test.BaseJUnit4ClassRunner.TraceFile')

_EXTRA_TEST_LIST = (
    'org.chromium.base.test.BaseChromiumAndroidJUnitRunner.TestList')

FEATURE_ANNOTATION = 'Feature'
RENDER_TEST_FEATURE_ANNOTATION = 'RenderTest'

# This needs to be kept in sync with formatting in |RenderUtils.imageName|
RE_RENDER_IMAGE_NAME = re.compile(
      r'(?P<test_class>\w+)\.'
      r'(?P<description>[-\w]+)\.'
      r'(?P<device_model_sdk>[-\w]+)\.png')

@contextlib.contextmanager
def _LogTestEndpoints(device, test_name):
  device.RunShellCommand(
      ['log', '-p', 'i', '-t', _TAG, 'START %s' % test_name],
      check_return=True)
  try:
    yield
  finally:
    device.RunShellCommand(
        ['log', '-p', 'i', '-t', _TAG, 'END %s' % test_name],
        check_return=True)

# TODO(jbudorick): Make this private once the instrumentation test_runner
# is deprecated.
def DidPackageCrashOnDevice(package_name, device):
  # Dismiss any error dialogs. Limit the number in case we have an error
  # loop or we are failing to dismiss.
  try:
    for _ in xrange(10):
      package = device.DismissCrashDialogIfNeeded(timeout=10, retries=1)
      if not package:
        return False
      # Assume test package convention of ".test" suffix
      if package in package_name:
        return True
  except device_errors.CommandFailedError:
    logging.exception('Error while attempting to dismiss crash dialog.')
  return False


_CURRENT_FOCUS_CRASH_RE = re.compile(
    r'\s*mCurrentFocus.*Application (Error|Not Responding): (\S+)}')


class LocalDeviceInstrumentationTestRun(
    local_device_test_run.LocalDeviceTestRun):
  def __init__(self, env, test_instance):
    super(LocalDeviceInstrumentationTestRun, self).__init__(
        env, test_instance)
    self._flag_changers = {}
    self._replace_package_contextmanager = None
    self._shared_prefs_to_restore = []

  #override
  def TestPackage(self):
    return self._test_instance.suite

  #override
  def SetUp(self):
    @local_device_environment.handle_shard_failures_with(
        self._env.BlacklistDevice)
    @trace_event.traced
    def individual_device_set_up(device, host_device_tuples):
      steps = []

      if self._test_instance.replace_system_package:
        @trace_event.traced
        def replace_package(dev):
          # We need the context manager to be applied before modifying any
          # shared preference files in case the replacement APK needs to be
          # set up, and it needs to be applied while the test is running.
          # Thus, it needs to be applied early during setup, but must still be
          # applied during _RunTest, which isn't possible using 'with' without
          # applying the context manager up in test_runner. Instead, we
          # manually invoke its __enter__ and __exit__ methods in setup and
          # teardown.
          self._replace_package_contextmanager = system_app.ReplaceSystemApp(
              dev, self._test_instance.replace_system_package.package,
              self._test_instance.replace_system_package.replacement_apk)
          # Pylint is not smart enough to realize that this field has
          # an __enter__ method, and will complain loudly.
          # pylint: disable=no-member
          self._replace_package_contextmanager.__enter__()
          # pylint: enable=no-member

        steps.append(replace_package)

      def install_helper(apk, permissions):
        @instrumentation_tracing.no_tracing
        @trace_event.traced("apk_path")
        def install_helper_internal(d, apk_path=apk.path):
          # pylint: disable=unused-argument
          d.Install(apk, permissions=permissions)
        return install_helper_internal

      def incremental_install_helper(apk, json_path, permissions):
        @trace_event.traced("apk_path")
        def incremental_install_helper_internal(d, apk_path=apk.path):
          # pylint: disable=unused-argument
          installer.Install(d, json_path, apk=apk, permissions=permissions)
        return incremental_install_helper_internal

      if self._test_instance.apk_under_test:
        permissions = self._test_instance.apk_under_test.GetPermissions()
        if self._test_instance.apk_under_test_incremental_install_json:
          steps.append(incremental_install_helper(
                           self._test_instance.apk_under_test,
                           self._test_instance.
                               apk_under_test_incremental_install_json,
                           permissions))
        else:
          steps.append(install_helper(self._test_instance.apk_under_test,
                                      permissions))

      permissions = self._test_instance.test_apk.GetPermissions()
      if self._test_instance.test_apk_incremental_install_json:
        steps.append(incremental_install_helper(
                         self._test_instance.test_apk,
                         self._test_instance.
                             test_apk_incremental_install_json,
                         permissions))
      else:
        steps.append(install_helper(self._test_instance.test_apk,
                                    permissions))

      steps.extend(install_helper(apk, None)
                   for apk in self._test_instance.additional_apks)

      @trace_event.traced
      def set_debug_app(dev):
        # Set debug app in order to enable reading command line flags on user
        # builds
        package_name = None
        if self._test_instance.apk_under_test:
          package_name = self._test_instance.apk_under_test.GetPackageName()
        elif self._test_instance.test_apk:
          package_name = self._test_instance.test_apk.GetPackageName()
        else:
          logging.error("Couldn't set debug app: no package name found")
          return
        cmd = ['am', 'set-debug-app', '--persistent']
        if self._test_instance.wait_for_java_debugger:
          cmd.append('-w')
        cmd.append(package_name)
        dev.RunShellCommand(cmd, check_return=True)

      @trace_event.traced
      def edit_shared_prefs(dev):
        for setting in self._test_instance.edit_shared_prefs:
          shared_pref = shared_prefs.SharedPrefs(
              dev, setting['package'], setting['filename'],
              use_encrypted_path=setting.get('supports_encrypted_path', False))
          pref_to_restore = copy.copy(shared_pref)
          pref_to_restore.Load()
          self._shared_prefs_to_restore.append(pref_to_restore)

          shared_preference_utils.ApplySharedPreferenceSetting(
              shared_pref, setting)

      @instrumentation_tracing.no_tracing
      def push_test_data(dev):
        device_root = posixpath.join(dev.GetExternalStoragePath(),
                                     'chromium_tests_root')
        host_device_tuples_substituted = [
            (h, local_device_test_run.SubstituteDeviceRoot(d, device_root))
            for h, d in host_device_tuples]
        logging.info('instrumentation data deps:')
        for h, d in host_device_tuples_substituted:
          logging.info('%r -> %r', h, d)
        dev.PushChangedFiles(host_device_tuples_substituted,
                             delete_device_stale=True)
        if not host_device_tuples_substituted:
          dev.RunShellCommand(['rm', '-rf', device_root], check_return=True)
          dev.RunShellCommand(['mkdir', '-p', device_root], check_return=True)

      @trace_event.traced
      def create_flag_changer(dev):
        if self._test_instance.flags:
          self._CreateFlagChangerIfNeeded(dev)
          logging.debug('Attempting to set flags: %r',
                        self._test_instance.flags)
          self._flag_changers[str(dev)].AddFlags(self._test_instance.flags)

        valgrind_tools.SetChromeTimeoutScale(
            dev, self._test_instance.timeout_scale)

      steps += [set_debug_app, edit_shared_prefs, push_test_data,
                create_flag_changer]

      def bind_crash_handler(step, dev):
        return lambda: crash_handler.RetryOnSystemCrash(step, dev)

      steps = [bind_crash_handler(s, device) for s in steps]

      try:
        if self._env.concurrent_adb:
          reraiser_thread.RunAsync(steps)
        else:
          for step in steps:
            step()
        if self._test_instance.store_tombstones:
          tombstones.ClearAllTombstones(device)
      except device_errors.CommandFailedError:
        # A bugreport can be large and take a while to generate, so only capture
        # one if we're using a remote manager.
        if isinstance(
            self._env.output_manager,
            remote_output_manager.RemoteOutputManager):
          logging.error(
              'Error when setting up device for tests. Taking a bugreport for '
              'investigation. This may take a while...')
          report_name = '%s.bugreport' % device.serial
          with self._env.output_manager.ArchivedTempfile(
              report_name, 'bug_reports') as report_file:
            device.TakeBugReport(report_file.name)
          logging.error('Bug report saved to %s', report_file.Link())
        raise

    self._env.parallel_devices.pMap(
        individual_device_set_up,
        self._test_instance.GetDataDependencies())
    if self._test_instance.wait_for_java_debugger:
      logging.warning('*' * 80)
      logging.warning('Waiting for debugger to attach to process: %s',
                      self._test_instance.apk_under_test.GetPackageName())
      logging.warning('*' * 80)

  #override
  def TearDown(self):
    @local_device_environment.handle_shard_failures_with(
        self._env.BlacklistDevice)
    @trace_event.traced
    def individual_device_tear_down(dev):
      if str(dev) in self._flag_changers:
        self._flag_changers[str(dev)].Restore()

      # Remove package-specific configuration
      dev.RunShellCommand(['am', 'clear-debug-app'], check_return=True)

      valgrind_tools.SetChromeTimeoutScale(dev, None)

      # Restore any shared preference files that we stored during setup.
      # This should be run sometime before the replace package contextmanager
      # gets exited so we don't have to special case restoring files of
      # replaced system apps.
      for pref_to_restore in self._shared_prefs_to_restore:
        pref_to_restore.Commit(force_commit=True)

      if self._replace_package_contextmanager:
        # See pylint-related commend above with __enter__()
        # pylint: disable=no-member
        self._replace_package_contextmanager.__exit__(*sys.exc_info())
        # pylint: enable=no-member

    self._env.parallel_devices.pMap(individual_device_tear_down)

  def _CreateFlagChangerIfNeeded(self, device):
    if str(device) not in self._flag_changers:
      self._flag_changers[str(device)] = flag_changer.FlagChanger(
        device, "test-cmdline-file")

  #override
  def _CreateShards(self, tests):
    return tests

  #override
  def _GetTests(self):
    if self._test_instance.junit4_runner_supports_listing:
      raw_tests = self._GetTestsFromRunner()
      tests = self._test_instance.ProcessRawTests(raw_tests)
    else:
      tests = self._test_instance.GetTests()
    tests = self._ApplyExternalSharding(
        tests, self._test_instance.external_shard_index,
        self._test_instance.total_external_shards)
    return tests

  #override
  def _GetUniqueTestName(self, test):
    return instrumentation_test_instance.GetUniqueTestName(test)

  #override
  def _RunTest(self, device, test):
    extras = {}

    flags_to_add = []
    test_timeout_scale = None
    if self._test_instance.coverage_directory:
      coverage_basename = '%s.ec' % ('%s_group' % test[0]['method']
          if isinstance(test, list) else test['method'])
      extras['coverage'] = 'true'
      coverage_directory = os.path.join(
          device.GetExternalStoragePath(), 'chrome', 'test', 'coverage')
      coverage_device_file = os.path.join(
          coverage_directory, coverage_basename)
      extras['coverageFile'] = coverage_device_file
    # Save screenshot if screenshot dir is specified (save locally) or if
    # a GS bucket is passed (save in cloud).
    screenshot_device_file = device_temp_file.DeviceTempFile(
        device.adb, suffix='.png', dir=device.GetExternalStoragePath())
    extras[EXTRA_SCREENSHOT_FILE] = screenshot_device_file.name

    # Set up the screenshot directory. This needs to be done for each test so
    # that we only get screenshots created by that test. It has to be on
    # external storage since the default location doesn't allow file creation
    # from the instrumentation test app on Android L and M.
    ui_capture_dir = device_temp_file.NamedDeviceTemporaryDirectory(
        device.adb,
        dir=device.GetExternalStoragePath())
    extras[EXTRA_UI_CAPTURE_DIR] = ui_capture_dir.name

    if self._env.trace_output:
      trace_device_file = device_temp_file.DeviceTempFile(
          device.adb, suffix='.json', dir=device.GetExternalStoragePath())
      extras[EXTRA_TRACE_FILE] = trace_device_file.name

    if isinstance(test, list):
      if not self._test_instance.driver_apk:
        raise Exception('driver_apk does not exist. '
                        'Please build it and try again.')
      if any(t.get('is_junit4') for t in test):
        raise Exception('driver apk does not support JUnit4 tests')

      def name_and_timeout(t):
        n = instrumentation_test_instance.GetTestName(t)
        i = self._GetTimeoutFromAnnotations(t['annotations'], n)
        return (n, i)

      test_names, timeouts = zip(*(name_and_timeout(t) for t in test))

      test_name = ','.join(test_names)
      test_display_name = test_name
      target = '%s/%s' % (
          self._test_instance.driver_package,
          self._test_instance.driver_name)
      extras.update(
          self._test_instance.GetDriverEnvironmentVars(
              test_list=test_names))
      timeout = sum(timeouts)
    else:
      test_name = instrumentation_test_instance.GetTestName(test)
      test_display_name = self._GetUniqueTestName(test)
      if test['is_junit4']:
        target = '%s/%s' % (
            self._test_instance.test_package,
            self._test_instance.junit4_runner_class)
      else:
        target = '%s/%s' % (
            self._test_instance.test_package,
            self._test_instance.junit3_runner_class)
      extras['class'] = test_name
      if 'flags' in test and test['flags']:
        flags_to_add.extend(test['flags'])
      timeout = self._GetTimeoutFromAnnotations(
        test['annotations'], test_display_name)

      test_timeout_scale = self._GetTimeoutScaleFromAnnotations(
          test['annotations'])
      if test_timeout_scale and test_timeout_scale != 1:
        valgrind_tools.SetChromeTimeoutScale(
            device, test_timeout_scale * self._test_instance.timeout_scale)

    if self._test_instance.wait_for_java_debugger:
      timeout = None
    logging.info('preparing to run %s: %s', test_display_name, test)

    render_tests_device_output_dir = None
    if _IsRenderTest(test):
      # TODO(mikecase): Add DeviceTempDirectory class and use that instead.
      render_tests_device_output_dir = posixpath.join(
          device.GetExternalStoragePath(),
          'render_test_output_dir')
      flags_to_add.append('--render-test-output-dir=%s' %
                          render_tests_device_output_dir)

    if flags_to_add:
      self._CreateFlagChangerIfNeeded(device)
      self._flag_changers[str(device)].PushFlags(add=flags_to_add)

    time_ms = lambda: int(time.time() * 1e3)
    start_ms = time_ms()

    stream_name = 'logcat_%s_%s_%s' % (
        test_name.replace('#', '.'),
        time.strftime('%Y%m%dT%H%M%S-UTC', time.gmtime()),
        device.serial)

    with ui_capture_dir:
      with self._env.output_manager.ArchivedTempfile(
          stream_name, 'logcat') as logcat_file:
        try:
          with logcat_monitor.LogcatMonitor(
              device.adb,
              filter_specs=local_device_environment.LOGCAT_FILTERS,
              output_file=logcat_file.name,
              transform_func=self._test_instance.MaybeDeobfuscateLines
              ) as logmon:
            with _LogTestEndpoints(device, test_name):
              with contextlib_ext.Optional(
                  trace_event.trace(test_name),
                  self._env.trace_output):
                output = device.StartInstrumentation(
                    target, raw=True, extras=extras, timeout=timeout, retries=0)
        finally:
          logmon.Close()

      if logcat_file.Link():
        logging.info('Logcat saved to %s', logcat_file.Link())

      duration_ms = time_ms() - start_ms

      with contextlib_ext.Optional(
          trace_event.trace('ProcessResults'),
          self._env.trace_output):
        output = self._test_instance.MaybeDeobfuscateLines(output)
        # TODO(jbudorick): Make instrumentation tests output a JSON so this
        # doesn't have to parse the output.
        result_code, result_bundle, statuses = (
            self._test_instance.ParseAmInstrumentRawOutput(output))
        results = self._test_instance.GenerateTestResults(
            result_code, result_bundle, statuses, start_ms, duration_ms,
            device.product_cpu_abi, self._test_instance.symbolizer)

      if self._env.trace_output:
        self._SaveTraceData(trace_device_file, device, test['class'])

      def restore_flags():
        if flags_to_add:
          self._flag_changers[str(device)].Restore()

      def restore_timeout_scale():
        if test_timeout_scale:
          valgrind_tools.SetChromeTimeoutScale(
              device, self._test_instance.timeout_scale)

      def handle_coverage_data():
        if self._test_instance.coverage_directory:
          device.PullFile(coverage_directory,
              self._test_instance.coverage_directory)
          device.RunShellCommand(
              'rm -f %s' % posixpath.join(coverage_directory, '*'),
              check_return=True, shell=True)

      def handle_render_test_data():
        if _IsRenderTest(test):
          # Render tests do not cause test failure by default. So we have to
          # check to see if any failure images were generated even if the test
          # does not fail.
          try:
            self._ProcessRenderTestResults(
                device, render_tests_device_output_dir, results)
          finally:
            device.RemovePath(render_tests_device_output_dir,
                              recursive=True, force=True)

      def pull_ui_screen_captures():
        screenshots = []
        for filename in device.ListDirectory(ui_capture_dir.name):
          if filename.endswith('.json'):
            screenshots.append(pull_ui_screenshot(filename))
        if screenshots:
          json_archive_name = 'ui_capture_%s_%s.json' % (
              test_name.replace('#', '.'),
              time.strftime('%Y%m%dT%H%M%S-UTC', time.gmtime()))
          with self._env.output_manager.ArchivedTempfile(
              json_archive_name, 'ui_capture', output_manager.Datatype.JSON
              ) as json_archive:
            json.dump(screenshots, json_archive)
          for result in results:
            result.SetLink('ui screenshot', json_archive.Link())

      def pull_ui_screenshot(filename):
        source_dir = ui_capture_dir.name
        json_path = posixpath.join(source_dir, filename)
        json_data = json.loads(device.ReadFile(json_path))
        image_file_path = posixpath.join(source_dir, json_data['location'])
        with self._env.output_manager.ArchivedTempfile(
            json_data['location'], 'ui_capture', output_manager.Datatype.PNG
            ) as image_archive:
          device.PullFile(image_file_path, image_archive.name)
        json_data['image_link'] = image_archive.Link()
        return json_data

      # While constructing the TestResult objects, we can parallelize several
      # steps that involve ADB. These steps should NOT depend on any info in
      # the results! Things such as whether the test CRASHED have not yet been
      # determined.
      post_test_steps = [restore_flags, restore_timeout_scale,
                         handle_coverage_data, handle_render_test_data,
                         pull_ui_screen_captures]
      if self._env.concurrent_adb:
        post_test_step_thread_group = reraiser_thread.ReraiserThreadGroup(
            reraiser_thread.ReraiserThread(f) for f in post_test_steps)
        post_test_step_thread_group.StartAll(will_block=True)
      else:
        for step in post_test_steps:
          step()

    for result in results:
      if logcat_file:
        result.SetLink('logcat', logcat_file.Link())

    # Update the result name if the test used flags.
    if flags_to_add:
      for r in results:
        if r.GetName() == test_name:
          r.SetName(test_display_name)

    # Add UNKNOWN results for any missing tests.
    iterable_test = test if isinstance(test, list) else [test]
    test_names = set(self._GetUniqueTestName(t) for t in iterable_test)
    results_names = set(r.GetName() for r in results)
    results.extend(
        base_test_result.BaseTestResult(u, base_test_result.ResultType.UNKNOWN)
        for u in test_names.difference(results_names))

    # Update the result type if we detect a crash.
    try:
      if DidPackageCrashOnDevice(self._test_instance.test_package, device):
        for r in results:
          if r.GetType() == base_test_result.ResultType.UNKNOWN:
            r.SetType(base_test_result.ResultType.CRASH)
    except device_errors.CommandTimeoutError:
      logging.warning('timed out when detecting/dismissing error dialogs')
      # Attach screenshot to the test to help with debugging the dialog boxes.
      self._SaveScreenshot(device, screenshot_device_file, test_display_name,
                           results, 'dialog_box_screenshot')

    # Handle failures by:
    #   - optionally taking a screenshot
    #   - logging the raw output at INFO level
    #   - clearing the application state while persisting permissions
    if any(r.GetType() not in (base_test_result.ResultType.PASS,
                               base_test_result.ResultType.SKIP)
           for r in results):
      self._SaveScreenshot(device, screenshot_device_file, test_display_name,
                           results, 'post_test_screenshot')

      logging.info('detected failure in %s. raw output:', test_display_name)
      for l in output:
        logging.info('  %s', l)
      if (not self._env.skip_clear_data
          and self._test_instance.package_info):
        permissions = (
            self._test_instance.apk_under_test.GetPermissions()
            if self._test_instance.apk_under_test
            else None)
        device.ClearApplicationState(self._test_instance.package_info.package,
                                     permissions=permissions)
    else:
      logging.debug('raw output from %s:', test_display_name)
      for l in output:
        logging.debug('  %s', l)
    if self._test_instance.store_tombstones:
      tombstones_url = None
      for result in results:
        if result.GetType() == base_test_result.ResultType.CRASH:
          if not tombstones_url:
            resolved_tombstones = tombstones.ResolveTombstones(
                device,
                resolve_all_tombstones=True,
                include_stack_symbols=False,
                wipe_tombstones=True,
                tombstone_symbolizer=self._test_instance.symbolizer)
            tombstone_filename = 'tombstones_%s_%s' % (
                time.strftime('%Y%m%dT%H%M%S-UTC', time.gmtime()),
                device.serial)
            with self._env.output_manager.ArchivedTempfile(
                tombstone_filename, 'tombstones') as tombstone_file:
              tombstone_file.write('\n'.join(resolved_tombstones))
            result.SetLink('tombstones', tombstone_file.Link())
    if self._env.concurrent_adb:
      post_test_step_thread_group.JoinAll()
    return results, None

  def _GetTestsFromRunner(self):
    test_apk_path = self._test_instance.test_apk.path
    pickle_path = '%s-runner.pickle' % test_apk_path
    # For incremental APKs, the code doesn't live in the apk, so instead check
    # the timestamp of the target's .stamp file.
    if self._test_instance.test_apk_incremental_install_json:
      with open(self._test_instance.test_apk_incremental_install_json) as f:
        data = json.load(f)
      out_dir = constants.GetOutDirectory()
      test_mtime = max(
          os.path.getmtime(os.path.join(out_dir, p)) for p in data['dex_files'])
    else:
      test_mtime = os.path.getmtime(test_apk_path)

    try:
      return instrumentation_test_instance.GetTestsFromPickle(
          pickle_path, test_mtime)
    except instrumentation_test_instance.TestListPickleException as e:
      logging.info('Could not get tests from pickle: %s', e)
    logging.info('Getting tests by having %s list them.',
                 self._test_instance.junit4_runner_class)
    def list_tests(d):
      def _run(dev):
        with device_temp_file.DeviceTempFile(
            dev.adb, suffix='.json',
            dir=dev.GetExternalStoragePath()) as dev_test_list_json:
          junit4_runner_class = self._test_instance.junit4_runner_class
          test_package = self._test_instance.test_package
          extras = {
            'log': 'true',
            # Workaround for https://github.com/mockito/mockito/issues/922
            'notPackage': 'net.bytebuddy',
          }
          extras[_EXTRA_TEST_LIST] = dev_test_list_json.name
          target = '%s/%s' % (test_package, junit4_runner_class)
          timeout = 120
          if self._test_instance.wait_for_java_debugger:
            timeout = None
          test_list_run_output = dev.StartInstrumentation(
              target, extras=extras, retries=0, timeout=timeout)
          if any(test_list_run_output):
            logging.error('Unexpected output while listing tests:')
            for line in test_list_run_output:
              logging.error('  %s', line)
          with tempfile_ext.NamedTemporaryDirectory() as host_dir:
            host_file = os.path.join(host_dir, 'list_tests.json')
            dev.PullFile(dev_test_list_json.name, host_file)
            with open(host_file, 'r') as host_file:
                return json.load(host_file)
      return crash_handler.RetryOnSystemCrash(_run, d)

    raw_test_lists = self._env.parallel_devices.pMap(list_tests).pGet(None)

    # If all devices failed to list tests, raise an exception.
    # Check that tl is not None and is not empty.
    if all(not tl for tl in raw_test_lists):
      raise device_errors.CommandFailedError(
          'Failed to list tests on any device')

    # Get the first viable list of raw tests
    raw_tests = [tl for tl in raw_test_lists if tl][0]

    instrumentation_test_instance.SaveTestsToPickle(pickle_path, raw_tests)
    return raw_tests

  def _SaveTraceData(self, trace_device_file, device, test_class):
    trace_host_file = self._env.trace_output

    if device.FileExists(trace_device_file.name):
      try:
        java_trace_json = device.ReadFile(trace_device_file.name)
      except IOError:
        raise Exception('error pulling trace file from device')
      finally:
        trace_device_file.close()

      process_name = '%s (device %s)' % (test_class, device.serial)
      process_hash = int(hashlib.md5(process_name).hexdigest()[:6], 16)

      java_trace = json.loads(java_trace_json)
      java_trace.sort(key=lambda event: event['ts'])

      get_date_command = 'echo $EPOCHREALTIME'
      device_time = device.RunShellCommand(get_date_command, single_line=True)
      device_time = float(device_time) * 1e6
      system_time = trace_time.Now()
      time_difference = system_time - device_time

      threads_to_add = set()
      for event in java_trace:
        # Ensure thread ID and thread name will be linked in the metadata.
        threads_to_add.add((event['tid'], event['name']))

        event['pid'] = process_hash

        # Adjust time stamp to align with Python trace times (from
        # trace_time.Now()).
        event['ts'] += time_difference

      for tid, thread_name in threads_to_add:
        thread_name_metadata = {'pid': process_hash, 'tid': tid,
                                'ts': 0, 'ph': 'M', 'cat': '__metadata',
                                'name': 'thread_name',
                                'args': {'name': thread_name}}
        java_trace.append(thread_name_metadata)

      process_name_metadata = {'pid': process_hash, 'tid': 0, 'ts': 0,
                               'ph': 'M', 'cat': '__metadata',
                               'name': 'process_name',
                               'args': {'name': process_name}}
      java_trace.append(process_name_metadata)

      java_trace_json = json.dumps(java_trace)
      java_trace_json = java_trace_json.rstrip(' ]')

      with open(trace_host_file, 'r') as host_handle:
        host_contents = host_handle.readline()

      if host_contents:
        java_trace_json = ',%s' % java_trace_json.lstrip(' [')

      with open(trace_host_file, 'a') as host_handle:
        host_handle.write(java_trace_json)

  def _SaveScreenshot(self, device, screenshot_device_file, test_name, results,
                      link_name):
      screenshot_filename = '%s-%s.png' % (
          test_name, time.strftime('%Y%m%dT%H%M%S-UTC', time.gmtime()))
      if device.FileExists(screenshot_device_file.name):
        with self._env.output_manager.ArchivedTempfile(
            screenshot_filename, 'screenshot',
            output_manager.Datatype.PNG) as screenshot_host_file:
          try:
            device.PullFile(screenshot_device_file.name,
                            screenshot_host_file.name)
          finally:
            screenshot_device_file.close()
        for result in results:
          result.SetLink(link_name, screenshot_host_file.Link())

  def _ProcessRenderTestResults(
      self, device, render_tests_device_output_dir, results):

    failure_images_device_dir = posixpath.join(
        render_tests_device_output_dir, 'failures')
    if not device.FileExists(failure_images_device_dir):
      return

    diff_images_device_dir = posixpath.join(
        render_tests_device_output_dir, 'diffs')

    golden_images_device_dir = posixpath.join(
        render_tests_device_output_dir, 'goldens')

    for failure_filename in device.ListDirectory(failure_images_device_dir):

      with self._env.output_manager.ArchivedTempfile(
          'fail_%s' % failure_filename, 'render_tests',
          output_manager.Datatype.PNG) as failure_image_host_file:
        device.PullFile(
            posixpath.join(failure_images_device_dir, failure_filename),
            failure_image_host_file.name)
      failure_link = failure_image_host_file.Link()

      golden_image_device_file = posixpath.join(
          golden_images_device_dir, failure_filename)
      if device.PathExists(golden_image_device_file):
        with self._env.output_manager.ArchivedTempfile(
            'golden_%s' % failure_filename, 'render_tests',
            output_manager.Datatype.PNG) as golden_image_host_file:
          device.PullFile(
              golden_image_device_file, golden_image_host_file.name)
        golden_link = golden_image_host_file.Link()
      else:
        golden_link = ''

      diff_image_device_file = posixpath.join(
          diff_images_device_dir, failure_filename)
      if device.PathExists(diff_image_device_file):
        with self._env.output_manager.ArchivedTempfile(
            'diff_%s' % failure_filename, 'render_tests',
            output_manager.Datatype.PNG) as diff_image_host_file:
          device.PullFile(
              diff_image_device_file, diff_image_host_file.name)
        diff_link = diff_image_host_file.Link()
      else:
        diff_link = ''

      jinja2_env = jinja2.Environment(
          loader=jinja2.FileSystemLoader(_JINJA_TEMPLATE_DIR),
          trim_blocks=True)
      template = jinja2_env.get_template(_JINJA_TEMPLATE_FILENAME)
      # pylint: disable=no-member
      processed_template_output = template.render(
          test_name=failure_filename,
          failure_link=failure_link,
          golden_link=golden_link,
          diff_link=diff_link)

      with self._env.output_manager.ArchivedTempfile(
          '%s.html' % failure_filename, 'render_tests',
          output_manager.Datatype.HTML) as html_results:
        html_results.write(processed_template_output)
        html_results.flush()
      for result in results:
        result.SetLink(failure_filename, html_results.Link())

  #override
  def _ShouldRetry(self, test, result):
    # We've tried to disable retries in the past with mixed results.
    # See crbug.com/619055 for historical context and crbug.com/797002
    # for ongoing efforts.
    del test, result
    return True

  #override
  def _ShouldShard(self):
    return True

  @classmethod
  def _GetTimeoutScaleFromAnnotations(cls, annotations):
    try:
      return int(annotations.get('TimeoutScale', {}).get('value', 1))
    except ValueError as e:
      logging.warning("Non-integer value of TimeoutScale ignored. (%s)", str(e))
      return 1

  @classmethod
  def _GetTimeoutFromAnnotations(cls, annotations, test_name):
    for k, v in TIMEOUT_ANNOTATIONS:
      if k in annotations:
        timeout = v
        break
    else:
      logging.warning('Using default 1 minute timeout for %s', test_name)
      timeout = 60

    timeout *= cls._GetTimeoutScaleFromAnnotations(annotations)

    return timeout


def _IsRenderTest(test):
  """Determines if a test or list of tests has a RenderTest amongst them."""
  if not isinstance(test, list):
    test = [test]
  return any([RENDER_TEST_FEATURE_ANNOTATION in t['annotations'].get(
              FEATURE_ANNOTATION, {}).get('value', ()) for t in test])
