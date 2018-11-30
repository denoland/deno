# Copyright (c) 2012 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Defines a set of constants shared by test runners and other scripts."""

# TODO(jbudorick): Split these constants into coherent modules.

# pylint: disable=W0212

import collections
import glob
import logging
import os
import subprocess

import devil.android.sdk.keyevent
from devil.android.constants import chrome
from devil.android.sdk import version_codes
from devil.constants import exit_codes


keyevent = devil.android.sdk.keyevent


DIR_SOURCE_ROOT = os.environ.get('CHECKOUT_SOURCE_ROOT',
    os.path.abspath(os.path.join(os.path.dirname(__file__),
                                 os.pardir, os.pardir, os.pardir, os.pardir)))

PACKAGE_INFO = dict(chrome.PACKAGE_INFO)
PACKAGE_INFO.update({
    'legacy_browser': chrome.PackageInfo(
        'com.google.android.browser',
        'com.android.browser.BrowserActivity',
        None,
        None),
    'chromecast_shell': chrome.PackageInfo(
        'com.google.android.apps.mediashell',
        'com.google.android.apps.mediashell.MediaShellActivity',
        'castshell-command-line',
        None),
    'android_webview_shell': chrome.PackageInfo(
        'org.chromium.android_webview.shell',
        'org.chromium.android_webview.shell.AwShellActivity',
        'android-webview-command-line',
        None),
    'gtest': chrome.PackageInfo(
        'org.chromium.native_test',
        'org.chromium.native_test.NativeUnitTestActivity',
        'chrome-native-tests-command-line',
        None),
    'components_browsertests': chrome.PackageInfo(
        'org.chromium.components_browsertests_apk',
        ('org.chromium.components_browsertests_apk' +
         '.ComponentsBrowserTestsActivity'),
        'chrome-native-tests-command-line',
        None),
    'content_browsertests': chrome.PackageInfo(
        'org.chromium.content_browsertests_apk',
        'org.chromium.content_browsertests_apk.ContentBrowserTestsActivity',
        'chrome-native-tests-command-line',
        None),
    'chromedriver_webview_shell': chrome.PackageInfo(
        'org.chromium.chromedriver_webview_shell',
        'org.chromium.chromedriver_webview_shell.Main',
        None,
        None),
})


# Ports arrangement for various test servers used in Chrome for Android.
# Lighttpd server will attempt to use 9000 as default port, if unavailable it
# will find a free port from 8001 - 8999.
LIGHTTPD_DEFAULT_PORT = 9000
LIGHTTPD_RANDOM_PORT_FIRST = 8001
LIGHTTPD_RANDOM_PORT_LAST = 8999
TEST_SYNC_SERVER_PORT = 9031
TEST_SEARCH_BY_IMAGE_SERVER_PORT = 9041
TEST_POLICY_SERVER_PORT = 9051


TEST_EXECUTABLE_DIR = '/data/local/tmp'
# Directories for common java libraries for SDK build.
# These constants are defined in build/android/ant/common.xml
SDK_BUILD_JAVALIB_DIR = 'lib.java'
SDK_BUILD_TEST_JAVALIB_DIR = 'test.lib.java'
SDK_BUILD_APKS_DIR = 'apks'

ADB_KEYS_FILE = '/data/misc/adb/adb_keys'

PERF_OUTPUT_DIR = os.path.join(DIR_SOURCE_ROOT, 'out', 'step_results')
# The directory on the device where perf test output gets saved to.
DEVICE_PERF_OUTPUT_DIR = (
    '/data/data/' + PACKAGE_INFO['chrome'].package + '/files')

SCREENSHOTS_DIR = os.path.join(DIR_SOURCE_ROOT, 'out_screenshots')

ANDROID_SDK_VERSION = version_codes.OREO_MR1
ANDROID_SDK_BUILD_TOOLS_VERSION = '27.0.3'
ANDROID_SDK_ROOT = os.path.join(DIR_SOURCE_ROOT,
                                'third_party', 'android_tools', 'sdk')
ANDROID_SDK_TOOLS = os.path.join(ANDROID_SDK_ROOT,
                                 'build-tools', ANDROID_SDK_BUILD_TOOLS_VERSION)
ANDROID_NDK_ROOT = os.path.join(DIR_SOURCE_ROOT,
                                'third_party', 'android_ndk')

BAD_DEVICES_JSON = os.path.join(DIR_SOURCE_ROOT,
                                os.environ.get('CHROMIUM_OUT_DIR', 'out'),
                                'bad_devices.json')

UPSTREAM_FLAKINESS_SERVER = 'test-results.appspot.com'

# TODO(jbudorick): Remove once unused.
DEVICE_LOCAL_PROPERTIES_PATH = '/data/local.prop'

# Configure ubsan to print stack traces in the format understood by "stack" so
# that they will be symbolized, and disable signal handlers because they
# interfere with the breakpad and sandbox tests.
# This value is duplicated in
# base/android/java/src/org/chromium/base/library_loader/LibraryLoader.java
UBSAN_OPTIONS = (
    'print_stacktrace=1 stack_trace_format=\'#%n pc %o %m\' '
    'handle_segv=0 handle_sigbus=0 handle_sigfpe=0')

# TODO(jbudorick): Rework this into testing/buildbot/
PYTHON_UNIT_TEST_SUITES = {
  'pylib_py_unittests': {
    'path': os.path.join(DIR_SOURCE_ROOT, 'build', 'android'),
    'test_modules': [
      'devil.android.device_utils_test',
      'devil.android.md5sum_test',
      'devil.utils.cmd_helper_test',
      'pylib.results.json_results_test',
      'pylib.utils.proguard_test',
    ]
  },
  'gyp_py_unittests': {
    'path': os.path.join(DIR_SOURCE_ROOT, 'build', 'android', 'gyp'),
    'test_modules': [
      'java_cpp_enum_tests',
      'java_google_api_keys_tests',
      'extract_unwind_tables_tests',
    ]
  },
}

LOCAL_MACHINE_TESTS = ['junit', 'python']
VALID_ENVIRONMENTS = ['local']
VALID_TEST_TYPES = ['gtest', 'instrumentation', 'junit', 'linker', 'monkey',
                    'perf', 'python']
VALID_DEVICE_TYPES = ['Android', 'iOS']


def SetBuildType(build_type):
  """Set the BUILDTYPE environment variable.

  NOTE: Using this function is deprecated, in favor of SetOutputDirectory(),
        it is still maintained for a few scripts that typically call it
        to implement their --release and --debug command-line options.

        When writing a new script, consider supporting an --output-dir or
        --chromium-output-dir option instead, and calling SetOutputDirectory()
        instead.

  NOTE: If CHROMIUM_OUTPUT_DIR if defined, or if SetOutputDirectory() was
  called previously, this will be completely ignored.
  """
  chromium_output_dir = os.environ.get('CHROMIUM_OUTPUT_DIR')
  if chromium_output_dir:
    logging.warning(
        'SetBuildType("%s") ignored since CHROMIUM_OUTPUT_DIR is already '
        'defined as (%s)', build_type, chromium_output_dir)
  os.environ['BUILDTYPE'] = build_type


def SetOutputDirectory(output_directory):
  """Set the Chromium output directory.

  This must be called early by scripts that rely on GetOutDirectory() or
  CheckOutputDirectory(). Typically by providing an --output-dir or
  --chromium-output-dir option.
  """
  os.environ['CHROMIUM_OUTPUT_DIR'] = output_directory


# The message that is printed when the Chromium output directory cannot
# be found. Note that CHROMIUM_OUT_DIR and BUILDTYPE are not mentioned
# intentionally to encourage the use of CHROMIUM_OUTPUT_DIR instead.
_MISSING_OUTPUT_DIR_MESSAGE = '\
The Chromium output directory could not be found. Please use an option such as \
--output-directory to provide it (see --help for details). Otherwise, \
define the CHROMIUM_OUTPUT_DIR environment variable.'


def GetOutDirectory():
  """Returns the Chromium build output directory.

  NOTE: This is determined in the following way:
    - From a previous call to SetOutputDirectory()
    - Otherwise, from the CHROMIUM_OUTPUT_DIR env variable, if it is defined.
    - Otherwise, from the current Chromium source directory, and a previous
      call to SetBuildType() or the BUILDTYPE env variable, in combination
      with the optional CHROMIUM_OUT_DIR env variable.
  """
  if 'CHROMIUM_OUTPUT_DIR' in os.environ:
    return os.path.abspath(os.path.join(
        DIR_SOURCE_ROOT, os.environ.get('CHROMIUM_OUTPUT_DIR')))

  build_type = os.environ.get('BUILDTYPE')
  if not build_type:
    raise EnvironmentError(_MISSING_OUTPUT_DIR_MESSAGE)

  return os.path.abspath(os.path.join(
      DIR_SOURCE_ROOT, os.environ.get('CHROMIUM_OUT_DIR', 'out'),
      build_type))


def CheckOutputDirectory():
  """Checks that the Chromium output directory is set, or can be found.

  If it is not already set, this will also perform a little auto-detection:

    - If the current directory contains a build.ninja file, use it as
      the output directory.

    - If CHROME_HEADLESS is defined in the environment (e.g. on a bot),
      look if there is a single output directory under DIR_SOURCE_ROOT/out/,
      and if so, use it as the output directory.

  Raises:
    Exception: If no output directory is detected.
  """
  output_dir = os.environ.get('CHROMIUM_OUTPUT_DIR')
  if output_dir:
    return

  build_type = os.environ.get('BUILDTYPE')
  if build_type and len(build_type) > 1:
    return

  # If CWD is an output directory, then assume it's the desired one.
  if os.path.exists('build.ninja'):
    output_dir = os.getcwd()
    SetOutputDirectory(output_dir)
    return

  # When running on bots, see if the output directory is obvious.
  # TODO(http://crbug.com/833808): Get rid of this by ensuring bots always set
  # CHROMIUM_OUTPUT_DIR correctly.
  if os.environ.get('CHROME_HEADLESS'):
    dirs = glob.glob(os.path.join(DIR_SOURCE_ROOT, 'out', '*', 'build.ninja'))
    if len(dirs) == 1:
      SetOutputDirectory(dirs[0])
      return

    raise Exception(
        'Chromium output directory not set, and CHROME_HEADLESS detected. ' +
        'However, multiple out dirs exist: %r' % dirs)

  raise Exception(_MISSING_OUTPUT_DIR_MESSAGE)


# Exit codes
ERROR_EXIT_CODE = exit_codes.ERROR
INFRA_EXIT_CODE = exit_codes.INFRA
WARNING_EXIT_CODE = exit_codes.WARNING
