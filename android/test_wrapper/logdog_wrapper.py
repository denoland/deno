#!/usr/bin/env python
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Wrapper for adding logdog streaming support to swarming tasks."""

import argparse
import contextlib
import logging
import os
import signal
import subprocess
import sys

_SRC_PATH = os.path.abspath(os.path.join(
    os.path.dirname(__file__), '..', '..', '..'))
sys.path.append(os.path.join(_SRC_PATH, 'third_party', 'catapult', 'devil'))
sys.path.append(os.path.join(_SRC_PATH, 'third_party', 'catapult', 'common',
                             'py_utils'))

from devil.utils import signal_handler
from devil.utils import timeout_retry
from py_utils import tempfile_ext

PROJECT = 'chromium'
OUTPUT = 'logdog'
COORDINATOR_HOST = 'luci-logdog.appspot.com'
SERVICE_ACCOUNT_JSON = ('/creds/service_accounts'
                        '/service-account-luci-logdog-publisher.json')
LOGDOG_TERMINATION_TIMEOUT = 30


def CommandParser():
  # Parses the command line arguments being passed in
  parser = argparse.ArgumentParser()
  parser.add_argument('--target', required=True,
                      help='The test target to be run.')
  parser.add_argument('--logdog-bin-cmd', required=True,
                      help='The logdog bin cmd.')
  return parser


def CreateStopTestsMethod(proc):
  def StopTests(signum, _frame):
    logging.error('Forwarding signal %s to test process', str(signum))
    proc.send_signal(signum)
  return StopTests


@contextlib.contextmanager
def NoLeakingProcesses(popen):
  try:
    yield popen
  finally:
    if popen is not None:
      try:
        if popen.poll() is None:
          popen.kill()
      except OSError:
        logging.warning('Failed to kill %s. Process may be leaked.',
                        str(popen.pid))


def main():
  parser = CommandParser()
  args, extra_cmd_args = parser.parse_known_args(sys.argv[1:])

  logging.basicConfig(level=logging.INFO)
  test_cmd = [
      os.path.join('bin', 'run_%s' % args.target),
      '-v']

  test_env = dict(os.environ)
  logdog_cmd = []

  with tempfile_ext.NamedTemporaryDirectory(
      prefix='tmp_android_logdog_wrapper') as temp_directory:
    if not os.path.exists(args.logdog_bin_cmd):
      logging.error(
          'Logdog binary %s unavailable. Unable to create logdog client',
          args.logdog_bin_cmd)
    else:
      streamserver_uri = 'unix:%s' % os.path.join(temp_directory,
                                                  'butler.sock')
      prefix = os.path.join('android', 'swarming', 'logcats',
                            os.environ.get('SWARMING_TASK_ID'))

      logdog_cmd = [
          args.logdog_bin_cmd,
          '-project', PROJECT,
          '-output', OUTPUT,
          '-prefix', prefix,
          '--service-account-json', SERVICE_ACCOUNT_JSON,
          '-coordinator-host', COORDINATOR_HOST,
          'serve',
          '-streamserver-uri', streamserver_uri]
      test_env.update({
          'LOGDOG_STREAM_PROJECT': PROJECT,
          'LOGDOG_STREAM_PREFIX': prefix,
          'LOGDOG_STREAM_SERVER_PATH': streamserver_uri,
          'LOGDOG_COORDINATOR_HOST': COORDINATOR_HOST,
      })

    test_cmd += extra_cmd_args

    logdog_proc = None
    if logdog_cmd:
      logdog_proc = subprocess.Popen(logdog_cmd)

    with NoLeakingProcesses(logdog_proc):
      with NoLeakingProcesses(
          subprocess.Popen(test_cmd, env=test_env)) as test_proc:
        with signal_handler.SignalHandler(signal.SIGTERM,
                                          CreateStopTestsMethod(test_proc)):
          result = test_proc.wait()
          if logdog_proc:
            def logdog_stopped():
              return logdog_proc.poll() is not None

            logdog_proc.terminate()
            timeout_retry.WaitFor(logdog_stopped, wait_period=1,
                                  max_tries=LOGDOG_TERMINATION_TIMEOUT)

            # If logdog_proc hasn't finished by this point, allow
            # NoLeakingProcesses to kill it.


  return result


if __name__ == '__main__':
  sys.exit(main())
