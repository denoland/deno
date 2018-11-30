#!/usr/bin/env python
#
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Runs all types of tests from one unified interface."""

import argparse
import collections
import contextlib
import itertools
import logging
import os
import shutil
import signal
import sys
import tempfile
import threading
import traceback
import unittest

# Import _strptime before threaded code. datetime.datetime.strptime is
# threadsafe except for the initial import of the _strptime module.
# See http://crbug.com/724524 and https://bugs.python.org/issue7980.
import _strptime  # pylint: disable=unused-import

# pylint: disable=ungrouped-imports
from pylib.constants import host_paths

if host_paths.DEVIL_PATH not in sys.path:
  sys.path.append(host_paths.DEVIL_PATH)

from devil import base_error
from devil.utils import reraiser_thread
from devil.utils import run_tests_helper

from pylib import constants
from pylib.base import base_test_result
from pylib.base import environment_factory
from pylib.base import output_manager
from pylib.base import output_manager_factory
from pylib.base import test_instance_factory
from pylib.base import test_run_factory
from pylib.results import json_results
from pylib.results import report_results
from pylib.results.presentation import test_results_presentation
from pylib.utils import logdog_helper
from pylib.utils import logging_utils
from pylib.utils import test_filter

from py_utils import contextlib_ext

_DEVIL_STATIC_CONFIG_FILE = os.path.abspath(os.path.join(
    host_paths.DIR_SOURCE_ROOT, 'build', 'android', 'devil_config.json'))


def _RealPath(arg):
  if arg.startswith('//'):
    arg = os.path.abspath(os.path.join(host_paths.DIR_SOURCE_ROOT,
                                       arg[2:].replace('/', os.sep)))
  return os.path.realpath(arg)


def AddTestLauncherOptions(parser):
  """Adds arguments mirroring //base/test/launcher.

  Args:
    parser: The parser to which arguments should be added.
  Returns:
    The given parser.
  """
  parser.add_argument(
      '--test-launcher-retry-limit',
      '--test_launcher_retry_limit',
      '--num_retries', '--num-retries',
      dest='num_retries', type=int, default=2,
      help='Number of retries for a test before '
           'giving up (default: %(default)s).')
  parser.add_argument(
      '--test-launcher-summary-output',
      '--json-results-file',
      dest='json_results_file', type=os.path.realpath,
      help='If set, will dump results in JSON form to the specified file. '
           'Note that this will also trigger saving per-test logcats to '
           'logdog.')
  parser.add_argument(
      '--test-launcher-shard-index',
      type=int, default=os.environ.get('GTEST_SHARD_INDEX', 0),
      help='Index of the external shard to run.')
  parser.add_argument(
      '--test-launcher-total-shards',
      type=int, default=os.environ.get('GTEST_TOTAL_SHARDS', 1),
      help='Total number of external shards.')

  test_filter.AddFilterOptions(parser)

  return parser


def AddCommandLineOptions(parser):
  """Adds arguments to support passing command-line flags to the device."""
  parser.add_argument(
      '--device-flags-file',
      type=os.path.realpath,
      help='The relative filepath to a file containing '
           'command-line flags to set on the device')
  parser.set_defaults(allow_unknown=True)
  parser.set_defaults(command_line_flags=None)


def AddTracingOptions(parser):
  # TODO(shenghuazhang): Move this into AddCommonOptions once it's supported
  # for all test types.
  parser.add_argument(
      '--trace-output',
      metavar='FILENAME', type=os.path.realpath,
      help='Path to save test_runner trace json output to.')

  parser.add_argument(
      '--trace-all',
      action='store_true',
      help='Whether to trace all function calls.')


def AddCommonOptions(parser):
  """Adds all common options to |parser|."""

  default_build_type = os.environ.get('BUILDTYPE', 'Debug')

  debug_or_release_group = parser.add_mutually_exclusive_group()
  debug_or_release_group.add_argument(
      '--debug',
      action='store_const', const='Debug', dest='build_type',
      default=default_build_type,
      help='If set, run test suites under out/Debug. '
           'Default is env var BUILDTYPE or Debug.')
  debug_or_release_group.add_argument(
      '--release',
      action='store_const', const='Release', dest='build_type',
      help='If set, run test suites under out/Release. '
           'Default is env var BUILDTYPE or Debug.')

  parser.add_argument(
      '--break-on-failure', '--break_on_failure',
      dest='break_on_failure', action='store_true',
      help='Whether to break on failure.')

  # TODO(jbudorick): Remove this once everything has switched to platform
  # mode.
  parser.add_argument(
      '--enable-platform-mode',
      action='store_true',
      help='Run the test scripts in platform mode, which '
           'conceptually separates the test runner from the '
           '"device" (local or remote, real or emulated) on '
           'which the tests are running. [experimental]')

  parser.add_argument(
      '-e', '--environment',
      default='local', choices=constants.VALID_ENVIRONMENTS,
      help='Test environment to run in (default: %(default)s).')

  parser.add_argument(
      '--local-output',
      action='store_true',
      help='Whether to archive test output locally and generate '
           'a local results detail page.')

  class FastLocalDevAction(argparse.Action):
    def __call__(self, parser, namespace, values, option_string=None):
      namespace.verbose_count = max(namespace.verbose_count, 1)
      namespace.num_retries = 0
      namespace.enable_device_cache = True
      namespace.enable_concurrent_adb = True
      namespace.skip_clear_data = True
      namespace.extract_test_list_from_filter = True

  parser.add_argument(
      '--fast-local-dev',
      type=bool, nargs=0, action=FastLocalDevAction,
      help='Alias for: --verbose --num-retries=0 '
           '--enable-device-cache --enable-concurrent-adb '
           '--skip-clear-data --extract-test-list-from-filter')

  # TODO(jbudorick): Remove this once downstream bots have switched to
  # api.test_results.
  parser.add_argument(
      '--flakiness-dashboard-server',
      dest='flakiness_dashboard_server',
      help=argparse.SUPPRESS)
  parser.add_argument(
      '--gs-results-bucket',
      help='Google Storage bucket to upload results to.')


  parser.add_argument(
      '--output-directory',
      dest='output_directory', type=os.path.realpath,
      help='Path to the directory in which build files are'
           ' located (must include build type). This will take'
           ' precedence over --debug and --release')
  parser.add_argument(
      '--repeat', '--gtest_repeat', '--gtest-repeat',
      dest='repeat', type=int, default=0,
      help='Number of times to repeat the specified set of tests.')
  parser.add_argument(
      '-v', '--verbose',
      dest='verbose_count', default=0, action='count',
      help='Verbose level (multiple times for more)')

  AddTestLauncherOptions(parser)


def ProcessCommonOptions(args):
  """Processes and handles all common options."""
  run_tests_helper.SetLogLevel(args.verbose_count, add_handler=False)
  # pylint: disable=redefined-variable-type
  if args.verbose_count > 0:
    handler = logging_utils.ColorStreamHandler()
  else:
    handler = logging.StreamHandler(sys.stdout)
  # pylint: enable=redefined-variable-type
  handler.setFormatter(run_tests_helper.CustomFormatter())
  logging.getLogger().addHandler(handler)

  constants.SetBuildType(args.build_type)
  if args.output_directory:
    constants.SetOutputDirectory(args.output_directory)


def AddDeviceOptions(parser):
  """Adds device options to |parser|."""

  parser = parser.add_argument_group('device arguments')

  parser.add_argument(
      '--adb-path',
      type=os.path.realpath,
      help='Specify the absolute path of the adb binary that '
           'should be used.')
  parser.add_argument(
      '--blacklist-file',
      type=os.path.realpath,
      help='Device blacklist file.')
  parser.add_argument(
      '-d', '--device', nargs='+',
      dest='test_devices',
      help='Target device(s) for the test suite to run on.')
  parser.add_argument(
      '--enable-concurrent-adb',
      action='store_true',
      help='Run multiple adb commands at the same time, even '
           'for the same device.')
  parser.add_argument(
      '--enable-device-cache',
      action='store_true',
      help='Cache device state to disk between runs')
  parser.add_argument(
      '--skip-clear-data',
      action='store_true',
      help='Do not wipe app data between tests. Use this to '
           'speed up local development and never on bots '
                     '(increases flakiness)')
  parser.add_argument(
      '--recover-devices',
      action='store_true',
      help='Attempt to recover devices prior to the final retry. Warning: '
           'this will cause all devices to reboot.')
  parser.add_argument(
      '--tool',
      dest='tool',
      help='Run the test under a tool '
           '(use --tool help to list them)')

  parser.add_argument(
      '--upload-logcats-file',
      action='store_true',
      dest='upload_logcats_file',
      help='Whether to upload logcat file to logdog.')

  logcat_output_group = parser.add_mutually_exclusive_group()
  logcat_output_group.add_argument(
      '--logcat-output-dir', type=os.path.realpath,
      help='If set, will dump logcats recorded during test run to directory. '
           'File names will be the device ids with timestamps.')
  logcat_output_group.add_argument(
      '--logcat-output-file', type=os.path.realpath,
      help='If set, will merge logcats recorded during test run and dump them '
           'to the specified file.')


def AddGTestOptions(parser):
  """Adds gtest options to |parser|."""

  parser = parser.add_argument_group('gtest arguments')

  parser.add_argument(
      '--app-data-file',
      action='append', dest='app_data_files',
      help='A file path relative to the app data directory '
           'that should be saved to the host.')
  parser.add_argument(
      '--app-data-file-dir',
      help='Host directory to which app data files will be'
           ' saved. Used with --app-data-file.')
  parser.add_argument(
      '--isolated-script-test-perf-output',
      help='If present, store chartjson results on this path.')
  parser.add_argument(
      '--delete-stale-data',
      dest='delete_stale_data', action='store_true',
      help='Delete stale test data on the device.')
  parser.add_argument(
      '--enable-xml-result-parsing',
      action='store_true', help=argparse.SUPPRESS)
  parser.add_argument(
      '--executable-dist-dir',
      type=os.path.realpath,
      help="Path to executable's dist directory for native"
           " (non-apk) tests.")
  parser.add_argument(
      '--extract-test-list-from-filter',
      action='store_true',
      help='When a test filter is specified, and the list of '
           'tests can be determined from it, skip querying the '
           'device for the list of all tests. Speeds up local '
           'development, but is not safe to use on bots ('
           'http://crbug.com/549214')
  parser.add_argument(
      '--gs-test-artifacts-bucket',
      help=('If present, test artifacts will be uploaded to this Google '
            'Storage bucket.'))
  parser.add_argument(
      '--gtest_also_run_disabled_tests', '--gtest-also-run-disabled-tests',
      dest='run_disabled', action='store_true',
      help='Also run disabled tests if applicable.')
  parser.add_argument(
      '--runtime-deps-path',
      dest='runtime_deps_path', type=os.path.realpath,
      help='Runtime data dependency file from GN.')
  parser.add_argument(
      '-t', '--shard-timeout',
      dest='shard_timeout', type=int, default=120,
      help='Timeout to wait for each test (default: %(default)s).')
  parser.add_argument(
      '--store-tombstones',
      dest='store_tombstones', action='store_true',
      help='Add tombstones in results if crash.')
  parser.add_argument(
      '-s', '--suite',
      dest='suite_name', nargs='+', metavar='SUITE_NAME', required=True,
      help='Executable name of the test suite to run.')
  parser.add_argument(
      '--test-apk-incremental-install-json',
      type=os.path.realpath,
      help='Path to install json for the test apk.')
  parser.add_argument(
      '-w', '--wait-for-java-debugger', action='store_true',
      help='Wait for java debugger to attach before running any application '
           'code. Also disables test timeouts and sets retries=0.')


def AddInstrumentationTestOptions(parser):
  """Adds Instrumentation test options to |parser|."""

  parser.add_argument_group('instrumentation arguments')

  parser.add_argument(
      '--additional-apk',
      action='append', dest='additional_apks', default=[],
      type=_RealPath,
      help='Additional apk that must be installed on '
           'the device when the tests are run')
  parser.add_argument(
      '-A', '--annotation',
      dest='annotation_str',
      help='Comma-separated list of annotations. Run only tests with any of '
           'the given annotations. An annotation can be either a key or a '
           'key-values pair. A test that has no annotation is considered '
           '"SmallTest".')
  # TODO(jbudorick): Remove support for name-style APK specification once
  # bots are no longer doing it.
  parser.add_argument(
      '--apk-under-test',
      help='Path or name of the apk under test.')
  parser.add_argument(
      '--coverage-dir',
      type=os.path.realpath,
      help='Directory in which to place all generated '
           'EMMA coverage files.')
  parser.add_argument(
      '--delete-stale-data',
      action='store_true', dest='delete_stale_data',
      help='Delete stale test data on the device.')
  parser.add_argument(
      '--disable-dalvik-asserts',
      dest='set_asserts', action='store_false', default=True,
      help='Removes the dalvik.vm.enableassertions property')
  parser.add_argument(
      '--enable-java-deobfuscation',
      action='store_true',
      help='Deobfuscate java stack traces in test output and logcat.')
  parser.add_argument(
      '-E', '--exclude-annotation',
      dest='exclude_annotation_str',
      help='Comma-separated list of annotations. Exclude tests with these '
           'annotations.')
  parser.add_argument(
      '--gtest_also_run_disabled_tests', '--gtest-also-run-disabled-tests',
      dest='run_disabled', action='store_true',
      help='Also run disabled tests if applicable.')
  def package_replacement(arg):
    split_arg = arg.split(',')
    if len(split_arg) != 2:
      raise argparse.ArgumentError(
          arg,
          'Expected two comma-separated strings for --replace-system-package, '
          'received %d' % len(split_arg))
    PackageReplacement = collections.namedtuple('PackageReplacement',
                                                ['package', 'replacement_apk'])
    return PackageReplacement(package=split_arg[0],
                              replacement_apk=_RealPath(split_arg[1]))
  parser.add_argument(
      '--replace-system-package',
      type=package_replacement, default=None,
      help='Specifies a system package to replace with a given APK for the '
           'duration of the test. Given as a comma-separated pair of strings, '
           'the first element being the package and the second the path to the '
           'replacement APK. Only supports replacing one package. Example: '
           '--replace-system-package com.example.app,path/to/some.apk')
  parser.add_argument(
      '--runtime-deps-path',
      dest='runtime_deps_path', type=os.path.realpath,
      help='Runtime data dependency file from GN.')
  parser.add_argument(
      '--screenshot-directory',
      dest='screenshot_dir', type=os.path.realpath,
      help='Capture screenshots of test failures')
  parser.add_argument(
      '--shared-prefs-file',
      dest='shared_prefs_file', type=_RealPath,
      help='The relative path to a file containing JSON list of shared '
           'preference files to edit and how to do so. Example list: '
           '[{'
           '  "package": "com.package.example",'
           '  "filename": "ExampleSettings.xml",'
           '  "set": {'
           '    "boolean_key_in_xml": true,'
           '    "string_key_in_xml": "string_value"'
           '  },'
           '  "remove": ['
           '    "key_in_xml_to_remove"'
           '  ]'
           '}]')
  parser.add_argument(
      '--store-tombstones',
      action='store_true', dest='store_tombstones',
      help='Add tombstones in results if crash.')
  parser.add_argument(
      '--strict-mode',
      dest='strict_mode', default='testing',
      help='StrictMode command-line flag set on the device, '
           'death/testing to kill the process, off to stop '
           'checking, flash to flash only. (default: %(default)s)')
  parser.add_argument(
      '--test-apk',
      required=True,
      help='Path or name of the apk containing the tests.')
  parser.add_argument(
      '--test-jar',
      help='Path of jar containing test java files.')
  parser.add_argument(
      '--timeout-scale',
      type=float,
      help='Factor by which timeouts should be scaled.')
  parser.add_argument(
      '-w', '--wait-for-java-debugger', action='store_true',
      help='Wait for java debugger to attach before running any application '
           'code. Also disables test timeouts and sets retries=0.')

  # These arguments are suppressed from the help text because they should
  # only ever be specified by an intermediate script.
  parser.add_argument(
      '--apk-under-test-incremental-install-json',
      help=argparse.SUPPRESS)
  parser.add_argument(
      '--test-apk-incremental-install-json',
      type=os.path.realpath,
      help=argparse.SUPPRESS)


def AddJUnitTestOptions(parser):
  """Adds junit test options to |parser|."""

  parser = parser.add_argument_group('junit arguments')

  parser.add_argument(
      '--coverage-dir', type=os.path.realpath,
      help='Directory to store coverage info.')
  parser.add_argument(
      '--package-filter',
      help='Filters tests by package.')
  parser.add_argument(
      '--runner-filter',
      help='Filters tests by runner class. Must be fully qualified.')
  parser.add_argument(
      '-s', '--test-suite', required=True,
      help='JUnit test suite to run.')
  debug_group = parser.add_mutually_exclusive_group()
  debug_group.add_argument(
      '-w', '--wait-for-java-debugger', action='store_const', const='8701',
      dest='debug_socket', help='Alias for --debug-socket=8701')
  debug_group.add_argument(
      '--debug-socket',
      help='Wait for java debugger to attach at specified socket address '
           'before running any application code. Also disables test timeouts '
           'and sets retries=0.')

  # These arguments are for Android Robolectric tests.
  parser.add_argument(
      '--android-manifest-path',
      help='Path to Android Manifest to configure Robolectric.')
  parser.add_argument(
      '--package-name',
      help='Default app package name for Robolectric tests.')
  parser.add_argument(
      '--resource-zip',
      action='append', dest='resource_zips', default=[],
      help='Path to resource zips to configure Robolectric.')
  parser.add_argument(
      '--robolectric-runtime-deps-dir',
      help='Path to runtime deps for Robolectric.')


def AddLinkerTestOptions(parser):

  parser.add_argument_group('linker arguments')

  parser.add_argument(
      '--test-apk',
      type=os.path.realpath,
      help='Path to the linker test APK.')


def AddMonkeyTestOptions(parser):
  """Adds monkey test options to |parser|."""

  parser = parser.add_argument_group('monkey arguments')

  parser.add_argument(
      '--browser',
      required=True, choices=constants.PACKAGE_INFO.keys(),
      metavar='BROWSER', help='Browser under test.')
  parser.add_argument(
      '--category',
      nargs='*', dest='categories', default=[],
      help='A list of allowed categories. Monkey will only visit activities '
           'that are listed with one of the specified categories.')
  parser.add_argument(
      '--event-count',
      default=10000, type=int,
      help='Number of events to generate (default: %(default)s).')
  parser.add_argument(
      '--seed',
      type=int,
      help='Seed value for pseudo-random generator. Same seed value generates '
           'the same sequence of events. Seed is randomized by default.')
  parser.add_argument(
      '--throttle',
      default=100, type=int,
      help='Delay between events (ms) (default: %(default)s). ')


def AddPerfTestOptions(parser):
  """Adds perf test options to |parser|."""

  parser = parser.add_argument_group('perf arguments')

  class SingleStepAction(argparse.Action):
    def __call__(self, parser, namespace, values, option_string=None):
      if values and not namespace.single_step:
        parser.error('single step command provided, '
                     'but --single-step not specified.')
      elif namespace.single_step and not values:
        parser.error('--single-step specified, '
                     'but no single step command provided.')
      setattr(namespace, self.dest, values)

  step_group = parser.add_mutually_exclusive_group(required=True)
  # TODO(jbudorick): Revise --single-step to use argparse.REMAINDER.
  # This requires removing "--" from client calls.
  step_group.add_argument(
      '--print-step',
      help='The name of a previously executed perf step to print.')
  step_group.add_argument(
      '--single-step',
      action='store_true',
      help='Execute the given command with retries, but only print the result '
           'for the "most successful" round.')
  step_group.add_argument(
      '--steps',
      help='JSON file containing the list of commands to run.')

  parser.add_argument(
      '--collect-chartjson-data',
      action='store_true',
      help='Cache the telemetry chartjson output from each step for later use.')
  parser.add_argument(
      '--dry-run',
      action='store_true',
      help='Just print the steps without executing.')
  # TODO(rnephew): Remove this when everything moves to new option in platform
  # mode.
  parser.add_argument(
      '--get-output-dir-archive',
      metavar='FILENAME', type=os.path.realpath,
      help='Write the cached output directory archived by a step into the'
      ' given ZIP file.')
  parser.add_argument(
      '--known-devices-file',
      help='Path to known device list.')
  # Uses 0.1 degrees C because that's what Android does.
  parser.add_argument(
      '--max-battery-temp',
      type=int,
      help='Only start tests when the battery is at or below the given '
           'temperature (0.1 C)')
  parser.add_argument(
      '--min-battery-level',
      type=int,
      help='Only starts tests when the battery is charged above '
           'given level.')
  parser.add_argument(
      '--no-timeout',
      action='store_true',
      help='Do not impose a timeout. Each perf step is responsible for '
           'implementing the timeout logic.')
  parser.add_argument(
      '--output-chartjson-data',
      type=os.path.realpath,
      help='Writes telemetry chartjson formatted output into the given file.')
  parser.add_argument(
      '--output-dir-archive-path',
      metavar='FILENAME', type=os.path.realpath,
      help='Write the cached output directory archived by a step into the'
      ' given ZIP file.')
  parser.add_argument(
      '--output-json-list',
      type=os.path.realpath,
      help='Writes a JSON list of information for each --steps into the given '
           'file. Information includes runtime and device affinity for each '
           '--steps.')
  parser.add_argument(
      '--write-buildbot-json',
      action='store_true',
      help='Whether to output buildbot json.')

  parser.add_argument(
      'single_step_command',
      nargs='*', action=SingleStepAction,
      help='If --single-step is specified, the command to run.')


def AddPythonTestOptions(parser):

  parser = parser.add_argument_group('python arguments')

  parser.add_argument(
      '-s', '--suite',
      dest='suite_name', metavar='SUITE_NAME',
      choices=constants.PYTHON_UNIT_TEST_SUITES.keys(),
      help='Name of the test suite to run.')


def _RunPythonTests(args):
  """Subcommand of RunTestsCommand which runs python unit tests."""
  suite_vars = constants.PYTHON_UNIT_TEST_SUITES[args.suite_name]
  suite_path = suite_vars['path']
  suite_test_modules = suite_vars['test_modules']

  sys.path = [suite_path] + sys.path
  try:
    suite = unittest.TestSuite()
    suite.addTests(unittest.defaultTestLoader.loadTestsFromName(m)
                   for m in suite_test_modules)
    runner = unittest.TextTestRunner(verbosity=1+args.verbose_count)
    return 0 if runner.run(suite).wasSuccessful() else 1
  finally:
    sys.path = sys.path[1:]


_DEFAULT_PLATFORM_MODE_TESTS = ['gtest', 'instrumentation', 'junit',
                                'linker', 'monkey', 'perf']


def RunTestsCommand(args):
  """Checks test type and dispatches to the appropriate function.

  Args:
    args: argparse.Namespace object.

  Returns:
    Integer indicated exit code.

  Raises:
    Exception: Unknown command name passed in, or an exception from an
        individual test runner.
  """
  command = args.command

  ProcessCommonOptions(args)
  logging.info('command: %s', ' '.join(sys.argv))
  if args.enable_platform_mode or command in _DEFAULT_PLATFORM_MODE_TESTS:
    return RunTestsInPlatformMode(args)

  if command == 'python':
    return _RunPythonTests(args)
  else:
    raise Exception('Unknown test type.')


_SUPPORTED_IN_PLATFORM_MODE = [
  # TODO(jbudorick): Add support for more test types.
  'gtest',
  'instrumentation',
  'junit',
  'linker',
  'monkey',
  'perf',
]


def RunTestsInPlatformMode(args):

  def infra_error(message):
    logging.fatal(message)
    sys.exit(constants.INFRA_EXIT_CODE)

  if args.command not in _SUPPORTED_IN_PLATFORM_MODE:
    infra_error('%s is not yet supported in platform mode' % args.command)

  ### Set up sigterm handler.

  def unexpected_sigterm(_signum, _frame):
    msg = [
      'Received SIGTERM. Shutting down.',
    ]
    for live_thread in threading.enumerate():
      # pylint: disable=protected-access
      thread_stack = ''.join(traceback.format_stack(
          sys._current_frames()[live_thread.ident]))
      msg.extend([
        'Thread "%s" (ident: %s) is currently running:' % (
            live_thread.name, live_thread.ident),
        thread_stack])

    infra_error('\n'.join(msg))

  signal.signal(signal.SIGTERM, unexpected_sigterm)

  ### Set up results handling.
  # TODO(jbudorick): Rewrite results handling.

  # all_raw_results is a list of lists of
  # base_test_result.TestRunResults objects. Each instance of
  # TestRunResults contains all test results produced by a single try,
  # while each list of TestRunResults contains all tries in a single
  # iteration.
  all_raw_results = []

  # all_iteration_results is a list of base_test_result.TestRunResults
  # objects. Each instance of TestRunResults contains the last test
  # result for each test run in that iteration.
  all_iteration_results = []

  global_results_tags = set()

  json_file = tempfile.NamedTemporaryFile(delete=False)
  json_file.close()

  @contextlib.contextmanager
  def json_finalizer():
    try:
      yield
    finally:
      if args.json_results_file and os.path.exists(json_file.name):
        shutil.move(json_file.name, args.json_results_file)
      else:
        os.remove(json_file.name)

  @contextlib.contextmanager
  def json_writer():
    try:
      yield
    except Exception:
      global_results_tags.add('UNRELIABLE_RESULTS')
      raise
    finally:
      json_results.GenerateJsonResultsFile(
          all_raw_results, json_file.name,
          global_tags=list(global_results_tags),
          indent=2)

  @contextlib.contextmanager
  def upload_logcats_file():
    try:
      yield
    finally:
      if not args.logcat_output_file:
        logging.critical('Cannot upload logcat file: no file specified.')
      elif not os.path.exists(args.logcat_output_file):
        logging.critical("Cannot upload logcat file: file doesn't exist.")
      else:
        with open(args.logcat_output_file) as src:
          dst = logdog_helper.open_text('unified_logcats')
          if dst:
            shutil.copyfileobj(src, dst)
            dst.close()
            logging.critical(
                'Logcat: %s', logdog_helper.get_viewer_url('unified_logcats'))


  logcats_uploader = contextlib_ext.Optional(
      upload_logcats_file(),
      'upload_logcats_file' in args and args.upload_logcats_file)

  ### Set up test objects.

  out_manager = output_manager_factory.CreateOutputManager(args)
  env = environment_factory.CreateEnvironment(
      args, out_manager, infra_error)
  test_instance = test_instance_factory.CreateTestInstance(args, infra_error)
  test_run = test_run_factory.CreateTestRun(
      args, env, test_instance, infra_error)

  ### Run.
  with out_manager, json_finalizer():
    with json_writer(), logcats_uploader, env, test_instance, test_run:

      repetitions = (xrange(args.repeat + 1) if args.repeat >= 0
                     else itertools.count())
      result_counts = collections.defaultdict(
          lambda: collections.defaultdict(int))
      iteration_count = 0
      for _ in repetitions:
        raw_results = test_run.RunTests()
        if not raw_results:
          continue

        all_raw_results.append(raw_results)

        iteration_results = base_test_result.TestRunResults()
        for r in reversed(raw_results):
          iteration_results.AddTestRunResults(r)
        all_iteration_results.append(iteration_results)

        iteration_count += 1
        for r in iteration_results.GetAll():
          result_counts[r.GetName()][r.GetType()] += 1
        report_results.LogFull(
            results=iteration_results,
            test_type=test_instance.TestType(),
            test_package=test_run.TestPackage(),
            annotation=getattr(args, 'annotations', None),
            flakiness_server=getattr(args, 'flakiness_dashboard_server',
                                     None))
        if args.break_on_failure and not iteration_results.DidRunPass():
          break

      if iteration_count > 1:
        # display summary results
        # only display results for a test if at least one test did not pass
        all_pass = 0
        tot_tests = 0
        for test_name in result_counts:
          tot_tests += 1
          if any(result_counts[test_name][x] for x in (
              base_test_result.ResultType.FAIL,
              base_test_result.ResultType.CRASH,
              base_test_result.ResultType.TIMEOUT,
              base_test_result.ResultType.UNKNOWN)):
            logging.critical(
                '%s: %s',
                test_name,
                ', '.join('%s %s' % (str(result_counts[test_name][i]), i)
                          for i in base_test_result.ResultType.GetTypes()))
          else:
            all_pass += 1

        logging.critical('%s of %s tests passed in all %s runs',
                         str(all_pass),
                         str(tot_tests),
                         str(iteration_count))

    if args.local_output:
      with out_manager.ArchivedTempfile(
          'test_results_presentation.html',
          'test_results_presentation',
          output_manager.Datatype.HTML) as results_detail_file:
        result_html_string, _, _ = test_results_presentation.result_details(
            json_path=json_file.name,
            test_name=args.command,
            cs_base_url='http://cs.chromium.org',
            local_output=True)
        results_detail_file.write(result_html_string)
        results_detail_file.flush()
      logging.critical('TEST RESULTS: %s', results_detail_file.Link())

      ui_screenshots = test_results_presentation.ui_screenshot_set(
          json_file.name)
      if ui_screenshots:
        with out_manager.ArchivedTempfile(
            'ui_screenshots.json',
            'ui_capture',
            output_manager.Datatype.JSON) as ui_screenshot_file:
          ui_screenshot_file.write(ui_screenshots)
        logging.critical('UI Screenshots: %s', ui_screenshot_file.Link())

  if args.command == 'perf' and (args.steps or args.single_step):
    return 0

  return (0 if all(r.DidRunPass() for r in all_iteration_results)
          else constants.ERROR_EXIT_CODE)


def DumpThreadStacks(_signal, _frame):
  for thread in threading.enumerate():
    reraiser_thread.LogThreadStack(thread)


def main():
  signal.signal(signal.SIGUSR1, DumpThreadStacks)

  parser = argparse.ArgumentParser()
  command_parsers = parser.add_subparsers(
      title='test types', dest='command')

  subp = command_parsers.add_parser(
      'gtest',
      help='googletest-based C++ tests')
  AddCommonOptions(subp)
  AddDeviceOptions(subp)
  AddGTestOptions(subp)
  AddTracingOptions(subp)
  AddCommandLineOptions(subp)

  subp = command_parsers.add_parser(
      'instrumentation',
      help='InstrumentationTestCase-based Java tests')
  AddCommonOptions(subp)
  AddDeviceOptions(subp)
  AddInstrumentationTestOptions(subp)
  AddTracingOptions(subp)
  AddCommandLineOptions(subp)

  subp = command_parsers.add_parser(
      'junit',
      help='JUnit4-based Java tests')
  AddCommonOptions(subp)
  AddJUnitTestOptions(subp)

  subp = command_parsers.add_parser(
      'linker',
      help='linker tests')
  AddCommonOptions(subp)
  AddDeviceOptions(subp)
  AddLinkerTestOptions(subp)

  subp = command_parsers.add_parser(
      'monkey',
      help="tests based on Android's monkey command")
  AddCommonOptions(subp)
  AddDeviceOptions(subp)
  AddMonkeyTestOptions(subp)

  subp = command_parsers.add_parser(
      'perf',
      help='performance tests')
  AddCommonOptions(subp)
  AddDeviceOptions(subp)
  AddPerfTestOptions(subp)
  AddTracingOptions(subp)

  subp = command_parsers.add_parser(
      'python',
      help='python tests based on unittest.TestCase')
  AddCommonOptions(subp)
  AddPythonTestOptions(subp)

  args, unknown_args = parser.parse_known_args()
  if unknown_args:
    if hasattr(args, 'allow_unknown') and args.allow_unknown:
      args.command_line_flags = unknown_args
    else:
      parser.error('unrecognized arguments: %s' % ' '.join(unknown_args))

  # --replace-system-package has the potential to cause issues if
  # --enable-concurrent-adb is set, so disallow that combination
  if (hasattr(args, 'replace_system_package') and
      hasattr(args, 'enable_concurrent_adb') and args.replace_system_package and
      args.enable_concurrent_adb):
    parser.error('--replace-system-package and --enable-concurrent-adb cannot '
                 'be used together')

  if (hasattr(args, 'debug_socket') or
      (hasattr(args, 'wait_for_java_debugger') and
      args.wait_for_java_debugger)):
    args.num_retries = 0

  try:
    return RunTestsCommand(args)
  except base_error.BaseError as e:
    logging.exception('Error occurred.')
    if e.is_infra_error:
      return constants.INFRA_EXIT_CODE
    return constants.ERROR_EXIT_CODE
  except: # pylint: disable=W0702
    logging.exception('Unrecognized error occurred.')
    return constants.ERROR_EXIT_CODE


if __name__ == '__main__':
  sys.exit(main())
