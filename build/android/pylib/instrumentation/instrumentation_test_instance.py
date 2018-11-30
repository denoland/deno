# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import copy
import logging
import os
import pickle
import re

from devil.android import apk_helper
from pylib import constants
from pylib.base import base_test_result
from pylib.base import test_exception
from pylib.base import test_instance
from pylib.constants import host_paths
from pylib.instrumentation import test_result
from pylib.instrumentation import instrumentation_parser
from pylib.symbols import deobfuscator
from pylib.symbols import stack_symbolizer
from pylib.utils import dexdump
from pylib.utils import instrumentation_tracing
from pylib.utils import proguard
from pylib.utils import shared_preference_utils
from pylib.utils import test_filter


with host_paths.SysPath(host_paths.BUILD_COMMON_PATH):
  import unittest_util # pylint: disable=import-error

# Ref: http://developer.android.com/reference/android/app/Activity.html
_ACTIVITY_RESULT_CANCELED = 0
_ACTIVITY_RESULT_OK = -1

_COMMAND_LINE_PARAMETER = 'cmdlinearg-parameter'
_DEFAULT_ANNOTATIONS = [
    'SmallTest', 'MediumTest', 'LargeTest', 'EnormousTest', 'IntegrationTest']
_EXCLUDE_UNLESS_REQUESTED_ANNOTATIONS = [
    'DisabledTest', 'FlakyTest', 'Manual']
_VALID_ANNOTATIONS = set(_DEFAULT_ANNOTATIONS +
                         _EXCLUDE_UNLESS_REQUESTED_ANNOTATIONS)

# These test methods are inherited from android.test base test class and
# should be permitted for not having size annotation. For more, please check
# https://developer.android.com/reference/android/test/AndroidTestCase.html
# https://developer.android.com/reference/android/test/ServiceTestCase.html
_TEST_WITHOUT_SIZE_ANNOTATIONS = [
    'testAndroidTestCaseSetupProperly', 'testServiceTestCaseSetUpProperly']

_EXTRA_DRIVER_TEST_LIST = (
    'org.chromium.test.driver.OnDeviceInstrumentationDriver.TestList')
_EXTRA_DRIVER_TEST_LIST_FILE = (
    'org.chromium.test.driver.OnDeviceInstrumentationDriver.TestListFile')
_EXTRA_DRIVER_TARGET_PACKAGE = (
    'org.chromium.test.driver.OnDeviceInstrumentationDriver.TargetPackage')
_EXTRA_DRIVER_TARGET_CLASS = (
    'org.chromium.test.driver.OnDeviceInstrumentationDriver.TargetClass')
_EXTRA_TIMEOUT_SCALE = (
    'org.chromium.test.driver.OnDeviceInstrumentationDriver.TimeoutScale')
_TEST_LIST_JUNIT4_RUNNERS = [
    'org.chromium.base.test.BaseChromiumAndroidJUnitRunner']

_SKIP_PARAMETERIZATION = 'SkipCommandLineParameterization'
_COMMANDLINE_PARAMETERIZATION = 'CommandLineParameter'
_NATIVE_CRASH_RE = re.compile('(process|native) crash', re.IGNORECASE)
_PICKLE_FORMAT_VERSION = 12


class MissingSizeAnnotationError(test_exception.TestException):
  def __init__(self, class_name):
    super(MissingSizeAnnotationError, self).__init__(class_name +
        ': Test method is missing required size annotation. Add one of: ' +
        ', '.join('@' + a for a in _VALID_ANNOTATIONS))


class TestListPickleException(test_exception.TestException):
  pass


# TODO(jbudorick): Make these private class methods of
# InstrumentationTestInstance once the instrumentation junit3_runner_class is
# deprecated.
def ParseAmInstrumentRawOutput(raw_output):
  """Parses the output of an |am instrument -r| call.

  Args:
    raw_output: the output of an |am instrument -r| call as a list of lines
  Returns:
    A 3-tuple containing:
      - the instrumentation code as an integer
      - the instrumentation result as a list of lines
      - the instrumentation statuses received as a list of 2-tuples
        containing:
        - the status code as an integer
        - the bundle dump as a dict mapping string keys to a list of
          strings, one for each line.
  """
  parser = instrumentation_parser.InstrumentationParser(raw_output)
  statuses = list(parser.IterStatus())
  code, bundle = parser.GetResult()
  return (code, bundle, statuses)


def GenerateTestResults(
    result_code, result_bundle, statuses, start_ms, duration_ms, device_abi,
    symbolizer):
  """Generate test results from |statuses|.

  Args:
    result_code: The overall status code as an integer.
    result_bundle: The summary bundle dump as a dict.
    statuses: A list of 2-tuples containing:
      - the status code as an integer
      - the bundle dump as a dict mapping string keys to string values
      Note that this is the same as the third item in the 3-tuple returned by
      |_ParseAmInstrumentRawOutput|.
    start_ms: The start time of the test in milliseconds.
    duration_ms: The duration of the test in milliseconds.
    device_abi: The device_abi, which is needed for symbolization.
    symbolizer: The symbolizer used to symbolize stack.

  Returns:
    A list containing an instance of InstrumentationTestResult for each test
    parsed.
  """

  results = []

  current_result = None

  for status_code, bundle in statuses:
    test_class = bundle.get('class', '')
    test_method = bundle.get('test', '')
    if test_class and test_method:
      test_name = '%s#%s' % (test_class, test_method)
    else:
      continue

    if status_code == instrumentation_parser.STATUS_CODE_START:
      if current_result:
        results.append(current_result)
      current_result = test_result.InstrumentationTestResult(
          test_name, base_test_result.ResultType.UNKNOWN, start_ms, duration_ms)
    else:
      if status_code == instrumentation_parser.STATUS_CODE_OK:
        if bundle.get('test_skipped', '').lower() in ('true', '1', 'yes'):
          current_result.SetType(base_test_result.ResultType.SKIP)
        elif current_result.GetType() == base_test_result.ResultType.UNKNOWN:
          current_result.SetType(base_test_result.ResultType.PASS)
      elif status_code == instrumentation_parser.STATUS_CODE_SKIP:
        current_result.SetType(base_test_result.ResultType.SKIP)
      elif status_code == instrumentation_parser.STATUS_CODE_ASSUMPTION_FAILURE:
        current_result.SetType(base_test_result.ResultType.SKIP)
      else:
        if status_code not in (instrumentation_parser.STATUS_CODE_ERROR,
                               instrumentation_parser.STATUS_CODE_FAILURE):
          logging.error('Unrecognized status code %d. Handling as an error.',
                        status_code)
        current_result.SetType(base_test_result.ResultType.FAIL)
    if 'stack' in bundle:
      if symbolizer and device_abi:
        current_result.SetLog(
            '%s\n%s' % (
              bundle['stack'],
              '\n'.join(symbolizer.ExtractAndResolveNativeStackTraces(
                  bundle['stack'], device_abi))))
      else:
        current_result.SetLog(bundle['stack'])

  if current_result:
    if current_result.GetType() == base_test_result.ResultType.UNKNOWN:
      crashed = (result_code == _ACTIVITY_RESULT_CANCELED
                 and any(_NATIVE_CRASH_RE.search(l)
                         for l in result_bundle.itervalues()))
      if crashed:
        current_result.SetType(base_test_result.ResultType.CRASH)

    results.append(current_result)

  return results


def FilterTests(tests, filter_str=None, annotations=None,
                excluded_annotations=None):
  """Filter a list of tests

  Args:
    tests: a list of tests. e.g. [
           {'annotations": {}, 'class': 'com.example.TestA', 'method':'test1'},
           {'annotations": {}, 'class': 'com.example.TestB', 'method':'test2'}]
    filter_str: googletest-style filter string.
    annotations: a dict of wanted annotations for test methods.
    exclude_annotations: a dict of annotations to exclude.

  Return:
    A list of filtered tests
  """
  def gtest_filter(t):
    if not filter_str:
      return True
    # Allow fully-qualified name as well as an omitted package.
    unqualified_class_test = {
      'class': t['class'].split('.')[-1],
      'method': t['method']
    }
    names = [
      GetTestName(t, sep='.'),
      GetTestName(unqualified_class_test, sep='.'),
      GetUniqueTestName(t, sep='.')
    ]

    if t['is_junit4']:
      names += [
          GetTestNameWithoutParameterPostfix(t, sep='.'),
          GetTestNameWithoutParameterPostfix(unqualified_class_test, sep='.')
      ]

    pattern_groups = filter_str.split('-')
    if len(pattern_groups) > 1:
      negative_filter = pattern_groups[1]
      if unittest_util.FilterTestNames(names, negative_filter):
        return []

    positive_filter = pattern_groups[0]
    return unittest_util.FilterTestNames(names, positive_filter)

  def annotation_filter(all_annotations):
    if not annotations:
      return True
    return any_annotation_matches(annotations, all_annotations)

  def excluded_annotation_filter(all_annotations):
    if not excluded_annotations:
      return True
    return not any_annotation_matches(excluded_annotations,
                                      all_annotations)

  def any_annotation_matches(filter_annotations, all_annotations):
    return any(
        ak in all_annotations
        and annotation_value_matches(av, all_annotations[ak])
        for ak, av in filter_annotations)

  def annotation_value_matches(filter_av, av):
    if filter_av is None:
      return True
    elif isinstance(av, dict):
      return filter_av in av['value']
    elif isinstance(av, list):
      return filter_av in av
    return filter_av == av

  filtered_tests = []
  for t in tests:
    # Gtest filtering
    if not gtest_filter(t):
      continue

    # Enforce that all tests declare their size.
    if (not any(a in _VALID_ANNOTATIONS for a in t['annotations'])
        and t['method'] not in _TEST_WITHOUT_SIZE_ANNOTATIONS):
      raise MissingSizeAnnotationError(GetTestName(t))

    if (not annotation_filter(t['annotations'])
        or not excluded_annotation_filter(t['annotations'])):
      continue

    filtered_tests.append(t)

  return filtered_tests


# TODO(yolandyan): remove this once the tests are converted to junit4
def GetAllTestsFromJar(test_jar):
  pickle_path = '%s-proguard.pickle' % test_jar
  try:
    tests = GetTestsFromPickle(pickle_path, os.path.getmtime(test_jar))
  except TestListPickleException as e:
    logging.info('Could not get tests from pickle: %s', e)
    logging.info('Getting tests from JAR via proguard.')
    tests = _GetTestsFromProguard(test_jar)
    SaveTestsToPickle(pickle_path, tests)
  return tests


def GetAllTestsFromApk(test_apk):
  pickle_path = '%s-dexdump.pickle' % test_apk
  try:
    tests = GetTestsFromPickle(pickle_path, os.path.getmtime(test_apk))
  except TestListPickleException as e:
    logging.info('Could not get tests from pickle: %s', e)
    logging.info('Getting tests from dex via dexdump.')
    tests = _GetTestsFromDexdump(test_apk)
    SaveTestsToPickle(pickle_path, tests)
  return tests

def GetTestsFromPickle(pickle_path, test_mtime):
  if not os.path.exists(pickle_path):
    raise TestListPickleException('%s does not exist.' % pickle_path)
  if os.path.getmtime(pickle_path) <= test_mtime:
    raise TestListPickleException('File is stale: %s' % pickle_path)

  with open(pickle_path, 'r') as f:
    pickle_data = pickle.load(f)
  if pickle_data['VERSION'] != _PICKLE_FORMAT_VERSION:
    raise TestListPickleException('PICKLE_FORMAT_VERSION has changed.')
  return pickle_data['TEST_METHODS']


# TODO(yolandyan): remove this once the test listing from java runner lands
@instrumentation_tracing.no_tracing
def _GetTestsFromProguard(jar_path):
  p = proguard.Dump(jar_path)
  class_lookup = dict((c['class'], c) for c in p['classes'])

  def is_test_class(c):
    return c['class'].endswith('Test')

  def is_test_method(m):
    return m['method'].startswith('test')

  def recursive_class_annotations(c):
    s = c['superclass']
    if s in class_lookup:
      a = recursive_class_annotations(class_lookup[s])
    else:
      a = {}
    a.update(c['annotations'])
    return a

  def stripped_test_class(c):
    return {
      'class': c['class'],
      'annotations': recursive_class_annotations(c),
      'methods': [m for m in c['methods'] if is_test_method(m)],
      'superclass': c['superclass'],
    }

  return [stripped_test_class(c) for c in p['classes']
          if is_test_class(c)]


def _GetTestsFromDexdump(test_apk):
  dump = dexdump.Dump(test_apk)
  tests = []

  def get_test_methods(methods):
    return [
        {
          'method': m,
          # No annotation info is available from dexdump.
          # Set MediumTest annotation for default.
          'annotations': {'MediumTest': None},
        } for m in methods if m.startswith('test')]

  for package_name, package_info in dump.iteritems():
    for class_name, class_info in package_info['classes'].iteritems():
      if class_name.endswith('Test'):
        tests.append({
            'class': '%s.%s' % (package_name, class_name),
            'annotations': {},
            'methods': get_test_methods(class_info['methods']),
            'superclass': class_info['superclass'],
        })
  return tests

def SaveTestsToPickle(pickle_path, tests):
  pickle_data = {
    'VERSION': _PICKLE_FORMAT_VERSION,
    'TEST_METHODS': tests,
  }
  with open(pickle_path, 'w') as pickle_file:
    pickle.dump(pickle_data, pickle_file)


class MissingJUnit4RunnerException(test_exception.TestException):
  """Raised when JUnit4 runner is not provided or specified in apk manifest"""

  def __init__(self):
    super(MissingJUnit4RunnerException, self).__init__(
        'JUnit4 runner is not provided or specified in test apk manifest.')


class UnmatchedFilterException(test_exception.TestException):
  """Raised when a user specifies a filter that doesn't match any tests."""

  def __init__(self, filter_str):
    super(UnmatchedFilterException, self).__init__(
        'Test filter "%s" matched no tests.' % filter_str)


def GetTestName(test, sep='#'):
  """Gets the name of the given test.

  Note that this may return the same name for more than one test, e.g. if a
  test is being run multiple times with different parameters.

  Args:
    test: the instrumentation test dict.
    sep: the character(s) that should join the class name and the method name.
  Returns:
    The test name as a string.
  """
  return '%s%s%s' % (test['class'], sep, test['method'])


def GetTestNameWithoutParameterPostfix(
      test, sep='#', parameterization_sep='__'):
  """Gets the name of the given JUnit4 test without parameter postfix.

  For most WebView JUnit4 javatests, each test is parameterizatized with
  "__sandboxed_mode" to run in both non-sandboxed mode and sandboxed mode.

  This function returns the name of the test without parameterization
  so test filters can match both parameterized and non-parameterized tests.

  Args:
    test: the instrumentation test dict.
    sep: the character(s) that should join the class name and the method name.
    parameterization_sep: the character(s) that seperate method name and method
                          parameterization postfix.
  Returns:
    The test name without parameter postfix as a string.
  """
  name = GetTestName(test, sep=sep)
  return name.split(parameterization_sep)[0]


def GetUniqueTestName(test, sep='#'):
  """Gets the unique name of the given test.

  This will include text to disambiguate between tests for which GetTestName
  would return the same name.

  Args:
    test: the instrumentation test dict.
    sep: the character(s) that should join the class name and the method name.
  Returns:
    The unique test name as a string.
  """
  display_name = GetTestName(test, sep=sep)
  if test.get('flags', [None])[0]:
    display_name = '%s with %s' % (display_name, ' '.join(test['flags']))
  return display_name


class InstrumentationTestInstance(test_instance.TestInstance):

  def __init__(self, args, data_deps_delegate, error_func):
    super(InstrumentationTestInstance, self).__init__()

    self._additional_apks = []
    self._apk_under_test = None
    self._apk_under_test_incremental_install_json = None
    self._package_info = None
    self._suite = None
    self._test_apk = None
    self._test_apk_incremental_install_json = None
    self._test_jar = None
    self._test_package = None
    self._junit3_runner_class = None
    self._junit4_runner_class = None
    self._junit4_runner_supports_listing = None
    self._test_support_apk = None
    self._initializeApkAttributes(args, error_func)

    self._data_deps = None
    self._data_deps_delegate = None
    self._runtime_deps_path = None
    self._initializeDataDependencyAttributes(args, data_deps_delegate)

    self._annotations = None
    self._excluded_annotations = None
    self._test_filter = None
    self._initializeTestFilterAttributes(args)

    self._flags = None
    self._initializeFlagAttributes(args)

    self._driver_apk = None
    self._driver_package = None
    self._driver_name = None
    self._initializeDriverAttributes()

    self._screenshot_dir = None
    self._timeout_scale = None
    self._wait_for_java_debugger = None
    self._initializeTestControlAttributes(args)

    self._coverage_directory = None
    self._initializeTestCoverageAttributes(args)

    self._store_tombstones = False
    self._symbolizer = None
    self._enable_java_deobfuscation = False
    self._deobfuscator = None
    self._initializeLogAttributes(args)

    self._edit_shared_prefs = []
    self._initializeEditPrefsAttributes(args)

    self._replace_system_package = None
    self._initializeReplaceSystemPackageAttributes(args)

    self._external_shard_index = args.test_launcher_shard_index
    self._total_external_shards = args.test_launcher_total_shards

  def _initializeApkAttributes(self, args, error_func):
    if args.apk_under_test:
      apk_under_test_path = args.apk_under_test
      if not args.apk_under_test.endswith('.apk'):
        apk_under_test_path = os.path.join(
            constants.GetOutDirectory(), constants.SDK_BUILD_APKS_DIR,
            '%s.apk' % args.apk_under_test)

      # TODO(jbudorick): Move the realpath up to the argument parser once
      # APK-by-name is no longer supported.
      apk_under_test_path = os.path.realpath(apk_under_test_path)

      if not os.path.exists(apk_under_test_path):
        error_func('Unable to find APK under test: %s' % apk_under_test_path)

      self._apk_under_test = apk_helper.ToHelper(apk_under_test_path)

    if args.test_apk.endswith('.apk'):
      self._suite = os.path.splitext(os.path.basename(args.test_apk))[0]
      test_apk_path = args.test_apk
      self._test_apk = apk_helper.ToHelper(args.test_apk)
    else:
      self._suite = args.test_apk
      test_apk_path = os.path.join(
          constants.GetOutDirectory(), constants.SDK_BUILD_APKS_DIR,
          '%s.apk' % args.test_apk)

    # TODO(jbudorick): Move the realpath up to the argument parser once
    # APK-by-name is no longer supported.
    test_apk_path = os.path.realpath(test_apk_path)

    if not os.path.exists(test_apk_path):
      error_func('Unable to find test APK: %s' % test_apk_path)

    self._test_apk = apk_helper.ToHelper(test_apk_path)

    self._apk_under_test_incremental_install_json = (
        args.apk_under_test_incremental_install_json)
    self._test_apk_incremental_install_json = (
        args.test_apk_incremental_install_json)

    if self._test_apk_incremental_install_json:
      assert self._suite.endswith('_incremental')
      self._suite = self._suite[:-len('_incremental')]

    self._test_jar = args.test_jar
    self._test_support_apk = apk_helper.ToHelper(os.path.join(
        constants.GetOutDirectory(), constants.SDK_BUILD_TEST_JAVALIB_DIR,
        '%sSupport.apk' % self._suite))

    if not os.path.exists(self._test_apk.path):
      error_func('Unable to find test APK: %s' % self._test_apk.path)
    if not self._test_jar:
      logging.warning('Test jar not specified. Test runner will not have '
                      'Java annotation info available. May not handle test '
                      'timeouts correctly.')
    elif not os.path.exists(self._test_jar):
      error_func('Unable to find test JAR: %s' % self._test_jar)

    self._test_package = self._test_apk.GetPackageName()
    all_instrumentations = self._test_apk.GetAllInstrumentations()
    all_junit3_runner_classes = [
        x for x in all_instrumentations if ('0xffffffff' in x.get(
            'chromium-junit3', ''))]
    all_junit4_runner_classes = [
        x for x in all_instrumentations if ('0xffffffff' not in x.get(
            'chromium-junit3', ''))]

    if len(all_junit3_runner_classes) > 1:
      logging.warning('This test apk has more than one JUnit3 instrumentation')
    if len(all_junit4_runner_classes) > 1:
      logging.warning('This test apk has more than one JUnit4 instrumentation')

    self._junit3_runner_class = (
      all_junit3_runner_classes[0]['android:name']
      if all_junit3_runner_classes else self.test_apk.GetInstrumentationName())

    self._junit4_runner_class = (
      all_junit4_runner_classes[0]['android:name']
      if all_junit4_runner_classes else None)

    if self._junit4_runner_class:
      if self._test_apk_incremental_install_json:
        self._junit4_runner_supports_listing = next(
            (True for x in self._test_apk.GetAllMetadata()
             if 'real-instr' in x[0] and x[1] in _TEST_LIST_JUNIT4_RUNNERS),
            False)
      else:
        self._junit4_runner_supports_listing = (
            self._junit4_runner_class in _TEST_LIST_JUNIT4_RUNNERS)

    self._package_info = None
    if self._apk_under_test:
      package_under_test = self._apk_under_test.GetPackageName()
      for package_info in constants.PACKAGE_INFO.itervalues():
        if package_under_test == package_info.package:
          self._package_info = package_info
          break
    if not self._package_info:
      logging.warning('Unable to find package info for %s', self._test_package)

    for apk in args.additional_apks:
      if not os.path.exists(apk):
        error_func('Unable to find additional APK: %s' % apk)
    self._additional_apks = (
        [apk_helper.ToHelper(x) for x in args.additional_apks])

  def _initializeDataDependencyAttributes(self, args, data_deps_delegate):
    self._data_deps = []
    self._data_deps_delegate = data_deps_delegate
    self._runtime_deps_path = args.runtime_deps_path

    if not self._runtime_deps_path:
      logging.warning('No data dependencies will be pushed.')

  def _initializeTestFilterAttributes(self, args):
    self._test_filter = test_filter.InitializeFilterFromArgs(args)

    def annotation_element(a):
      a = a.split('=', 1)
      return (a[0], a[1] if len(a) == 2 else None)

    if args.annotation_str:
      self._annotations = [
          annotation_element(a) for a in args.annotation_str.split(',')]
    elif not self._test_filter:
      self._annotations = [
          annotation_element(a) for a in _DEFAULT_ANNOTATIONS]
    else:
      self._annotations = []

    if args.exclude_annotation_str:
      self._excluded_annotations = [
          annotation_element(a) for a in args.exclude_annotation_str.split(',')]
    else:
      self._excluded_annotations = []

    requested_annotations = set(a[0] for a in self._annotations)
    if not args.run_disabled:
      self._excluded_annotations.extend(
          annotation_element(a) for a in _EXCLUDE_UNLESS_REQUESTED_ANNOTATIONS
          if a not in requested_annotations)

  def _initializeFlagAttributes(self, args):
    self._flags = ['--enable-test-intents']
    if args.command_line_flags:
      self._flags.extend(args.command_line_flags)
    if args.device_flags_file:
      with open(args.device_flags_file) as device_flags_file:
        stripped_lines = (l.strip() for l in device_flags_file)
        self._flags.extend(flag for flag in stripped_lines if flag)
    if args.strict_mode and args.strict_mode != 'off':
      self._flags.append('--strict-mode=' + args.strict_mode)

  def _initializeDriverAttributes(self):
    self._driver_apk = os.path.join(
        constants.GetOutDirectory(), constants.SDK_BUILD_APKS_DIR,
        'OnDeviceInstrumentationDriver.apk')
    if os.path.exists(self._driver_apk):
      driver_apk = apk_helper.ApkHelper(self._driver_apk)
      self._driver_package = driver_apk.GetPackageName()
      self._driver_name = driver_apk.GetInstrumentationName()
    else:
      self._driver_apk = None

  def _initializeTestControlAttributes(self, args):
    self._screenshot_dir = args.screenshot_dir
    self._timeout_scale = args.timeout_scale or 1
    self._wait_for_java_debugger = args.wait_for_java_debugger

  def _initializeTestCoverageAttributes(self, args):
    self._coverage_directory = args.coverage_dir

  def _initializeLogAttributes(self, args):
    self._enable_java_deobfuscation = args.enable_java_deobfuscation
    self._store_tombstones = args.store_tombstones
    self._symbolizer = stack_symbolizer.Symbolizer(
        self.apk_under_test.path if self.apk_under_test else None)

  def _initializeEditPrefsAttributes(self, args):
    if not hasattr(args, 'shared_prefs_file') or not args.shared_prefs_file:
      return
    if not isinstance(args.shared_prefs_file, str):
      logging.warning("Given non-string for a filepath")
      return
    self._edit_shared_prefs = shared_preference_utils.ExtractSettingsFromJson(
        args.shared_prefs_file)

  def _initializeReplaceSystemPackageAttributes(self, args):
    if (not hasattr(args, 'replace_system_package')
        or not args.replace_system_package):
      return
    self._replace_system_package = args.replace_system_package

  @property
  def additional_apks(self):
    return self._additional_apks

  @property
  def apk_under_test(self):
    return self._apk_under_test

  @property
  def apk_under_test_incremental_install_json(self):
    return self._apk_under_test_incremental_install_json

  @property
  def coverage_directory(self):
    return self._coverage_directory

  @property
  def driver_apk(self):
    return self._driver_apk

  @property
  def driver_package(self):
    return self._driver_package

  @property
  def driver_name(self):
    return self._driver_name

  @property
  def edit_shared_prefs(self):
    return self._edit_shared_prefs

  @property
  def external_shard_index(self):
    return self._external_shard_index

  @property
  def flags(self):
    return self._flags

  @property
  def junit3_runner_class(self):
    return self._junit3_runner_class

  @property
  def junit4_runner_class(self):
    return self._junit4_runner_class

  @property
  def junit4_runner_supports_listing(self):
    return self._junit4_runner_supports_listing

  @property
  def package_info(self):
    return self._package_info

  @property
  def replace_system_package(self):
    return self._replace_system_package

  @property
  def screenshot_dir(self):
    return self._screenshot_dir

  @property
  def store_tombstones(self):
    return self._store_tombstones

  @property
  def suite(self):
    return self._suite

  @property
  def symbolizer(self):
    return self._symbolizer

  @property
  def test_apk(self):
    return self._test_apk

  @property
  def test_apk_incremental_install_json(self):
    return self._test_apk_incremental_install_json

  @property
  def test_jar(self):
    return self._test_jar

  @property
  def test_support_apk(self):
    return self._test_support_apk

  @property
  def test_package(self):
    return self._test_package

  @property
  def timeout_scale(self):
    return self._timeout_scale

  @property
  def total_external_shards(self):
    return self._total_external_shards

  @property
  def wait_for_java_debugger(self):
    return self._wait_for_java_debugger

  #override
  def TestType(self):
    return 'instrumentation'

  #override
  def SetUp(self):
    self._data_deps.extend(
        self._data_deps_delegate(self._runtime_deps_path))
    if self._enable_java_deobfuscation:
      self._deobfuscator = deobfuscator.DeobfuscatorPool(
          self.test_apk.path + '.mapping')

  def GetDataDependencies(self):
    return self._data_deps

  def GetTests(self):
    if self.test_jar:
      raw_tests = GetAllTestsFromJar(self.test_jar)
    else:
      raw_tests = GetAllTestsFromApk(self.test_apk.path)
    return self.ProcessRawTests(raw_tests)

  def MaybeDeobfuscateLines(self, lines):
    if not self._deobfuscator:
      return lines
    return self._deobfuscator.TransformLines(lines)

  def ProcessRawTests(self, raw_tests):
    inflated_tests = self._ParameterizeTestsWithFlags(
        self._InflateTests(raw_tests))
    if self._junit4_runner_class is None and any(
        t['is_junit4'] for t in inflated_tests):
      raise MissingJUnit4RunnerException()
    filtered_tests = FilterTests(
        inflated_tests, self._test_filter, self._annotations,
        self._excluded_annotations)
    if self._test_filter and not filtered_tests:
      for t in inflated_tests:
        logging.debug('  %s', GetUniqueTestName(t))
      raise UnmatchedFilterException(self._test_filter)
    return filtered_tests

  # pylint: disable=no-self-use
  def _InflateTests(self, tests):
    inflated_tests = []
    for c in tests:
      for m in c['methods']:
        a = dict(c['annotations'])
        a.update(m['annotations'])
        inflated_tests.append({
            'class': c['class'],
            'method': m['method'],
            'annotations': a,
            'is_junit4': c['superclass'] == 'java.lang.Object'
        })
    return inflated_tests

  def _ParameterizeTestsWithFlags(self, tests):
    new_tests = []
    for t in tests:
      annotations = t['annotations']
      parameters = None
      if (annotations.get(_COMMANDLINE_PARAMETERIZATION)
          and _SKIP_PARAMETERIZATION not in annotations):
        parameters = annotations[_COMMANDLINE_PARAMETERIZATION]['value']
      if parameters:
        t['flags'] = [parameters[0]]
        for p in parameters[1:]:
          parameterized_t = copy.copy(t)
          parameterized_t['flags'] = ['--%s' % p]
          new_tests.append(parameterized_t)
    return tests + new_tests

  def GetDriverEnvironmentVars(
      self, test_list=None, test_list_file_path=None):
    env = {
      _EXTRA_DRIVER_TARGET_PACKAGE: self.test_package,
      _EXTRA_DRIVER_TARGET_CLASS: self.junit3_runner_class,
      _EXTRA_TIMEOUT_SCALE: self._timeout_scale,
    }

    if test_list:
      env[_EXTRA_DRIVER_TEST_LIST] = ','.join(test_list)

    if test_list_file_path:
      env[_EXTRA_DRIVER_TEST_LIST_FILE] = (
          os.path.basename(test_list_file_path))

    return env

  @staticmethod
  def ParseAmInstrumentRawOutput(raw_output):
    return ParseAmInstrumentRawOutput(raw_output)

  @staticmethod
  def GenerateTestResults(
      result_code, result_bundle, statuses, start_ms, duration_ms,
      device_abi, symbolizer):
    return GenerateTestResults(result_code, result_bundle, statuses,
                               start_ms, duration_ms, device_abi, symbolizer)

  #override
  def TearDown(self):
    self.symbolizer.CleanUp()
    if self._deobfuscator:
      self._deobfuscator.Close()
      self._deobfuscator = None
