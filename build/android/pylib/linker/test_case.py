# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Base class for linker-specific test cases.

   The custom dynamic linker can only be tested through a custom test case
   for various technical reasons:

     - It's an 'invisible feature', i.e. it doesn't expose a new API or
       behaviour, all it does is save RAM when loading native libraries.

     - Checking that it works correctly requires several things that do not
       fit the existing GTest-based and instrumentation-based tests:

         - Native test code needs to be run in both the browser and renderer
           process at the same time just after loading native libraries, in
           a completely asynchronous way.

         - Each test case requires restarting a whole new application process
           with a different command-line.

         - Enabling test support in the Linker code requires building a special
           APK with a flag to activate special test-only support code in the
           Linker code itself.

       Host-driven tests have also been tried, but since they're really
       sub-classes of instrumentation tests, they didn't work well either.

   To build and run the linker tests, do the following:

     ninja -C out/Debug chromium_linker_test_apk
     out/Debug/bin/run_chromium_linker_test_apk

"""
# pylint: disable=R0201

import logging
import re

from devil.android import device_errors
from devil.android.sdk import intent
from pylib.base import base_test_result


ResultType = base_test_result.ResultType

_PACKAGE_NAME = 'org.chromium.chromium_linker_test_apk'
_ACTIVITY_NAME = '.ChromiumLinkerTestActivity'
_COMMAND_LINE_FILE = '/data/local/tmp/chromium-linker-test-command-line'

# Logcat filters used during each test. Only the 'chromium' one is really
# needed, but the logs are added to the TestResult in case of error, and
# it is handy to have others as well when troubleshooting.
_LOGCAT_FILTERS = ['*:s', 'chromium:v', 'cr_chromium:v',
                   'cr_ChromiumAndroidLinker:v', 'cr_LibraryLoader:v',
                   'cr_LinkerTest:v']
#_LOGCAT_FILTERS = ['*:v']  ## DEBUG

# Regular expression used to match status lines in logcat.
_RE_BROWSER_STATUS_LINE = re.compile(r' BROWSER_LINKER_TEST: (FAIL|SUCCESS)$')
_RE_RENDERER_STATUS_LINE = re.compile(r' RENDERER_LINKER_TEST: (FAIL|SUCCESS)$')

def _StartActivityAndWaitForLinkerTestStatus(device, timeout):
  """Force-start an activity and wait up to |timeout| seconds until the full
     linker test status lines appear in the logcat, recorded through |device|.
  Args:
    device: A DeviceUtils instance.
    timeout: Timeout in seconds
  Returns:
    A (status, logs) tuple, where status is a ResultType constant, and logs
    if the final logcat output as a string.
  """

  # 1. Start recording logcat with appropriate filters.
  with device.GetLogcatMonitor(filter_specs=_LOGCAT_FILTERS) as logmon:

    # 2. Force-start activity.
    device.StartActivity(
        intent.Intent(package=_PACKAGE_NAME, activity=_ACTIVITY_NAME),
        force_stop=True)

    # 3. Wait up to |timeout| seconds until the test status is in the logcat.
    result = ResultType.PASS
    try:
      browser_match = logmon.WaitFor(_RE_BROWSER_STATUS_LINE, timeout=timeout)
      logging.debug('Found browser match: %s', browser_match.group(0))
      renderer_match = logmon.WaitFor(_RE_RENDERER_STATUS_LINE,
                                      timeout=timeout)
      logging.debug('Found renderer match: %s', renderer_match.group(0))
      if (browser_match.group(1) != 'SUCCESS'
          or renderer_match.group(1) != 'SUCCESS'):
        result = ResultType.FAIL
    except device_errors.CommandTimeoutError:
      result = ResultType.TIMEOUT

    logcat = device.adb.Logcat(dump=True)

  logmon.Close()
  return result, '\n'.join(logcat)


class LibraryLoadMap(dict):
  """A helper class to pretty-print a map of library names to load addresses."""
  def __str__(self):
    items = ['\'%s\': 0x%x' % (name, address) for \
        (name, address) in self.iteritems()]
    return '{%s}' % (', '.join(items))

  def __repr__(self):
    return 'LibraryLoadMap(%s)' % self.__str__()


class AddressList(list):
  """A helper class to pretty-print a list of load addresses."""
  def __str__(self):
    items = ['0x%x' % address for address in self]
    return '[%s]' % (', '.join(items))

  def __repr__(self):
    return 'AddressList(%s)' % self.__str__()


class LinkerTestCaseBase(object):
  """Base class for linker test cases."""

  def __init__(self, is_low_memory=False):
    """Create a test case.
    Args:
      is_low_memory: True to simulate a low-memory device, False otherwise.
    """
    test_suffix = 'ForLinker'
    self.is_low_memory = is_low_memory
    if is_low_memory:
      test_suffix += 'LowMemoryDevice'
    else:
      test_suffix += 'RegularDevice'
    class_name = self.__class__.__name__
    self.qualified_name = '%s.%s' % (class_name, test_suffix)
    self.tagged_name = self.qualified_name

  def _RunTest(self, _device):
    """Run the test, must be overriden.
    Args:
      _device: A DeviceUtils interface.
    Returns:
      A (status, log) tuple, where <status> is a ResultType constant, and <log>
      is the logcat output captured during the test in case of error, or None
      in case of success.
    """
    return ResultType.FAIL, 'Unimplemented _RunTest() method!'

  def Run(self, device):
    """Run the test on a given device.
    Args:
      device: Name of target device where to run the test.
    Returns:
      A base_test_result.TestRunResult() instance.
    """
    margin = 8
    print '[ %-*s ] %s' % (margin, 'RUN', self.tagged_name)
    logging.info('Running linker test: %s', self.tagged_name)

    command_line_flags = ''
    if self.is_low_memory:
      command_line_flags += ' --low-memory-device'
    device.WriteFile(_COMMAND_LINE_FILE, command_line_flags)

    # Run the test.
    status, logs = self._RunTest(device)

    result_text = 'OK'
    if status == ResultType.FAIL:
      result_text = 'FAILED'
    elif status == ResultType.TIMEOUT:
      result_text = 'TIMEOUT'
    print '[ %*s ] %s' % (margin, result_text, self.tagged_name)

    return base_test_result.BaseTestResult(self.tagged_name, status, log=logs)


  def __str__(self):
    return self.tagged_name

  def __repr__(self):
    return self.tagged_name


class LinkerSharedRelroTest(LinkerTestCaseBase):
  """A linker test case to check the status of shared RELRO sections.

    The core of the checks performed here are pretty simple:

      - Clear the logcat and start recording with an appropriate set of filters.
      - Create the command-line appropriate for the test-case.
      - Start the activity (always forcing a cold start).
      - Every second, look at the current content of the filtered logcat lines
        and look for instances of the following:

            BROWSER_LINKER_TEST: <status>
            RENDERER_LINKER_TEST: <status>

        where <status> can be either FAIL or SUCCESS. These lines can appear
        in any order in the logcat. Once both browser and renderer status are
        found, stop the loop. Otherwise timeout after 30 seconds.

        Note that there can be other lines beginning with BROWSER_LINKER_TEST:
        and RENDERER_LINKER_TEST:, but are not followed by a <status> code.

      - The test case passes if the <status> for both the browser and renderer
        process are SUCCESS. Otherwise its a fail.
  """
  def _RunTest(self, device):
    # Wait up to 30 seconds until the linker test status is in the logcat.
    return _StartActivityAndWaitForLinkerTestStatus(device, timeout=30)
