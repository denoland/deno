#!/usr/bin/env vpython
#
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import json
import logging
import os
import pipes
import re
import signal
import sys
import tempfile

import psutil  # pylint: disable=import-error

CHROMIUM_SRC_PATH = os.path.abspath(os.path.join(
    os.path.dirname(__file__), '..', '..'))

# Use the android test-runner's gtest results support library for generating
# output json ourselves.
sys.path.insert(0, os.path.join(CHROMIUM_SRC_PATH, 'build', 'android'))
from pylib.base import base_test_result  # pylint: disable=import-error
from pylib.results import json_results  # pylint: disable=import-error

# Use luci-py's subprocess42.py
sys.path.insert(
    0, os.path.join(CHROMIUM_SRC_PATH, 'tools', 'swarming_client', 'utils'))
import subprocess42  # pylint: disable=import-error

DEFAULT_CROS_CACHE = os.path.abspath(os.path.join(
    CHROMIUM_SRC_PATH, 'build', 'cros_cache'))
CHROMITE_PATH = os.path.abspath(os.path.join(
    CHROMIUM_SRC_PATH, 'third_party', 'chromite'))
CROS_RUN_VM_TEST_PATH = os.path.abspath(os.path.join(
    CHROMITE_PATH, 'bin', 'cros_run_vm_test'))

# GN target that corresponds to the cros browser sanity test.
SANITY_TEST_TARGET = 'cros_vm_sanity_test'


class TestFormatError(Exception):
  pass


class RemoteTest(object):

  def __init__(self, args, unknown_args):
    self._additional_args = unknown_args
    self._path_to_outdir = args.path_to_outdir
    self._test_launcher_summary_output = args.test_launcher_summary_output
    self._vm_logs_dir = args.vm_logs_dir

    self._retries = 0
    self._timeout = None

    self._vm_test_cmd = [
        CROS_RUN_VM_TEST_PATH,
        '--start',
        '--board', args.board,
        '--cache-dir', args.cros_cache,
        # Don't persist any filesystem changes after the VM shutsdown.
        '--copy-on-write',
    ]
    if args.vm_logs_dir:
      self._vm_test_cmd += [
          '--results-src', '/var/log/',
          '--results-dest-dir', args.vm_logs_dir,
      ]

    self._test_env = setup_env()

  @property
  def suite_name(self):
    raise NotImplementedError('Child classes need to define suite name.')

  @property
  def vm_test_cmd(self):
    return self._vm_test_cmd

  def run_test(self):
    # Traps SIGTERM and kills all child processes of cros_run_vm_test when it's
    # caught. This will allow us to capture logs from the VM if a test hangs
    # and gets timeout-killed by swarming. See also:
    # https://chromium.googlesource.com/infra/luci/luci-py/+/master/appengine/swarming/doc/Bot.md#graceful-termination_aka-the-sigterm-and-sigkill-dance
    test_proc = None
    def _kill_child_procs(trapped_signal, _):
      logging.warning(
          'Received signal %d. Killing child processes of test.',
          trapped_signal)
      if not test_proc or not test_proc.pid:
        # This shouldn't happen?
        logging.error('Test process not running.')
        return
      for child in psutil.Process(test_proc.pid).children():
        logging.warning('Killing process %s', child)
        child.kill()

    signal.signal(signal.SIGTERM, _kill_child_procs)

    for i in xrange(self._retries+1):
      logging.info('########################################')
      logging.info('Test attempt #%d', i)
      logging.info('########################################')
      test_proc = subprocess42.Popen(
          self._vm_test_cmd, stdout=sys.stdout, stderr=sys.stderr,
          env=self._test_env)
      try:
        test_proc.wait(timeout=self._timeout)
      except subprocess42.TimeoutExpired:
        logging.error('Test timed out. Sending SIGTERM.')
        # SIGTERM the proc and wait 10s for it to close.
        test_proc.terminate()
        try:
          test_proc.wait(timeout=10)
        except subprocess42.TimeoutExpired:
          # If it hasn't closed in 10s, SIGKILL it.
          logging.error('Test did not exit in time. Sending SIGKILL.')
          test_proc.kill()
          test_proc.wait()
      logging.info('Test exitted with %d.', test_proc.returncode)
      if test_proc.returncode == 0:
        break

    self.post_run(test_proc.returncode)
    return test_proc.returncode

  def post_run(self, return_code):
    # Create a simple json results file for a test run. The results will contain
    # only one test (suite_name), and will either be a PASS or FAIL depending on
    # return_code.
    if self._test_launcher_summary_output:
      result = (base_test_result.ResultType.FAIL if return_code else
                    base_test_result.ResultType.PASS)
      suite_result = base_test_result.BaseTestResult(self.suite_name, result)
      run_results = base_test_result.TestRunResults()
      run_results.AddResult(suite_result)
      with open(self._test_launcher_summary_output, 'w') as f:
        json.dump(json_results.GenerateResultsDict([run_results]), f)


class TastTest(RemoteTest):

  def __init__(self, args, unknown_args):
    super(TastTest, self).__init__(args, unknown_args)

    self._suite_name = args.suite_name
    self._tests = args.tests
    self._conditional = args.conditional

  @property
  def suite_name(self):
    return self._suite_name

  def build_test_command(self):
    if self._additional_args:
      raise TestFormatError(
          'Tast tests should not have additional args: %s' % (
              self._additional_args))

    self._vm_test_cmd += [
        '--deploy',
        '--build-dir', os.path.relpath(self._path_to_outdir, CHROMIUM_SRC_PATH),
        '--cmd',
        '--',
        'local_test_runner',
    ]
    if self._conditional:
      self._vm_test_cmd.append(pipes.quote(self._conditional))
    else:
      self._vm_test_cmd.extend(self._tests)


class GTestTest(RemoteTest):

  _FILE_BLACKLIST = [
      re.compile(r'.*build/chromeos.*'),
      re.compile(r'.*build/cros_cache.*'),
      re.compile(r'.*third_party/chromite.*'),
  ]

  def __init__(self, args, unknown_args):
    super(GTestTest, self).__init__(args, unknown_args)

    self._test_exe = args.test_exe
    self._runtime_deps_path = args.runtime_deps_path
    self._vpython_dir = args.vpython_dir

    self._test_launcher_shard_index = args.test_launcher_shard_index
    self._test_launcher_total_shards = args.test_launcher_total_shards

    self._on_vm_script = None

  @property
  def suite_name(self):
    return self._test_exe

  def build_test_command(self):
    # To keep things easy for us, ensure both types of output locations are
    # the same.
    if self._test_launcher_summary_output and self._vm_logs_dir:
      json_out_dir = os.path.dirname(self._test_launcher_summary_output) or '.'
      if os.path.abspath(json_out_dir) != os.path.abspath(self._vm_logs_dir):
        raise TestFormatError(
            '--test-launcher-summary-output and --vm-logs-dir must point to '
            'the same directory.')

    if self._test_launcher_summary_output:
      result_dir, result_file = os.path.split(
          self._test_launcher_summary_output)
      # If args.test_launcher_summary_output is a file in cwd, result_dir will
      # be an empty string, so replace it with '.' when this is the case so
      # cros_run_vm_test can correctly handle it.
      if not result_dir:
        result_dir = '.'
      vm_result_file = '/tmp/%s' % result_file
      self._vm_test_cmd += [
          '--results-src', vm_result_file,
          '--results-dest-dir', result_dir,
      ]

    # Build the shell script that will be used on the VM to invoke the test.
    vm_test_script_contents = ['#!/bin/sh']

    # /home is mounted with "noexec" in the VM, but some of our tools
    # and tests use the home dir as a workspace (eg: vpython downloads
    # python binaries to ~/.vpython-root). /tmp doesn't have this
    # restriction, so change the location of the home dir for the
    # duration of the test.
    vm_test_script_contents.append('export HOME=/tmp')

    if self._vpython_dir:
      vpython_spec_path = os.path.relpath(
          os.path.join(CHROMIUM_SRC_PATH, '.vpython'),
          self._path_to_outdir)
      # Initialize the vpython cache. This can take 10-20s, and some tests
      # can't afford to wait that long on the first invocation.
      vm_test_script_contents.extend([
          'export PATH=$PATH:$PWD/%s' % (self._vpython_dir),
          'vpython -vpython-spec %s -vpython-tool install' % (
              vpython_spec_path),
      ])

    test_invocation = (
        './%s --test-launcher-shard-index=%d '
        '--test-launcher-total-shards=%d' % (
            self._test_exe, self._test_launcher_shard_index,
            self._test_launcher_total_shards)
    )
    if self._test_launcher_summary_output:
      test_invocation += ' --test-launcher-summary-output=%s' % vm_result_file
    if self._additional_args:
      test_invocation += ' %s' % ' '.join(self._additional_args)
    vm_test_script_contents.append(test_invocation)

    logging.info('Running the following command in the VM:')
    logging.info('\n' + '\n'.join(vm_test_script_contents))
    fd, tmp_path = tempfile.mkstemp(suffix='.sh', dir=self._path_to_outdir)
    os.fchmod(fd, 0755)
    with os.fdopen(fd, 'wb') as f:
      f.write('\n'.join(vm_test_script_contents))
    self._on_vm_script = tmp_path

    runtime_files = [os.path.relpath(self._on_vm_script)]
    runtime_files += self._read_runtime_files()
    if self._vpython_dir:
      # --vpython-dir is relative to the out dir, but --files expects paths
      # relative to src dir, so fix the path up a bit.
      runtime_files.append(
          os.path.relpath(
              os.path.abspath(os.path.join(self._path_to_outdir,
                                           self._vpython_dir)),
              CHROMIUM_SRC_PATH))
      # TODO(bpastene): Add the vpython spec to the test's runtime deps instead
      # of handling it here.
      runtime_files.append('.vpython')

    # Since we're pushing files, we need to set the cwd.
    self._vm_test_cmd.extend(
        ['--cwd', os.path.relpath(self._path_to_outdir, CHROMIUM_SRC_PATH)])
    for f in runtime_files:
      self._vm_test_cmd.extend(['--files', f])

    self._vm_test_cmd += [
        # Some tests fail as root, so run as the less privileged user 'chronos'.
        '--as-chronos',
        '--cmd',
        '--',
        './' + os.path.relpath(self._on_vm_script, self._path_to_outdir)
    ]

  def _read_runtime_files(self):
    if not self._runtime_deps_path:
      return []

    abs_runtime_deps_path = os.path.abspath(
        os.path.join(self._path_to_outdir, self._runtime_deps_path))
    with open(abs_runtime_deps_path) as runtime_deps_file:
      files = [l.strip() for l in runtime_deps_file if l]
    rel_file_paths = []
    for f in files:
      rel_file_path = os.path.relpath(
          os.path.abspath(os.path.join(self._path_to_outdir, f)))
      if not any(regex.match(rel_file_path) for regex in self._FILE_BLACKLIST):
        rel_file_paths.append(rel_file_path)
    return rel_file_paths

  def post_run(self, _):
    if self._on_vm_script:
      os.remove(self._on_vm_script)


class BrowserSanityTest(RemoteTest):

  def __init__(self, args, unknown_args):
    super(BrowserSanityTest, self).__init__(args, unknown_args)

    # 10 min should be enough time for the sanity test to pass.
    self._retries = 1
    self._timeout = 600

  @property
  def suite_name(self):
    return SANITY_TEST_TARGET

  def build_test_command(self):
    if '--gtest_filter=%s' % SANITY_TEST_TARGET in self._additional_args:
      logging.info(
          'GTest filtering not supported for the sanity test. The '
          '--gtest_filter arg will be ignored.')
      self._additional_args.remove('--gtest_filter=%s' % SANITY_TEST_TARGET)

    if self._additional_args:
      raise TestFormatError(
          'Sanity test should not have additional args: %s' % (
              self._additional_args))

    # run_cros_vm_test's default behavior when no cmd is specified is the sanity
    # test that's baked into the VM image. This test smoke-checks the system
    # browser, so deploy our locally-built chrome to the VM before testing.
    self._vm_test_cmd += [
        '--deploy',
        '--build-dir', os.path.relpath(self._path_to_outdir, CHROMIUM_SRC_PATH),
    ]


def vm_test(args, unknown_args):
  # cros_run_vm_test has trouble with relative paths that go up directories,
  # so cd to src/, which should be the root of all data deps.
  os.chdir(CHROMIUM_SRC_PATH)

  # pylint: disable=redefined-variable-type
  # TODO: Remove the above when depot_tool's pylint is updated to include the
  # fix to https://github.com/PyCQA/pylint/issues/710.
  if args.test_type == 'tast':
    test = TastTest(args, unknown_args)
  elif args.test_exe == SANITY_TEST_TARGET:
    test = BrowserSanityTest(args, unknown_args)
  else:
    test = GTestTest(args, unknown_args)

  test.build_test_command()
  logging.info('Running the following command on the host:')
  logging.info(' '.join(test.vm_test_cmd))

  return test.run_test()


def host_cmd(args, unknown_args):
  if not args.cmd:
    raise TestFormatError('Must specify command to run on the host.')
  elif unknown_args:
    raise TestFormatError(
        'Args "%s" unsupported. Is your host command correctly formatted?' % (
            ' '.join(unknown_args)))
  elif args.deploy_chrome and not args.path_to_outdir:
    raise TestFormatError(
        '--path-to-outdir must be specified if --deploy-chrome is passed.')

  cros_run_vm_test_cmd = [
      CROS_RUN_VM_TEST_PATH,
      '--start',
      '--board', args.board,
      '--cache-dir', args.cros_cache,
      # Don't persist any filesystem changes after the VM shutsdown.
      '--copy-on-write',
  ]
  if args.verbose:
    cros_run_vm_test_cmd.append('--debug')

  test_env = setup_env()
  if args.deploy_chrome:
    cros_run_vm_test_cmd += [
        '--deploy',
        '--build-dir', os.path.abspath(args.path_to_outdir),
    ]

  cros_run_vm_test_cmd += [
      '--host-cmd',
      '--',
  ] + args.cmd

  logging.info('Running the following command:')
  logging.info(' '.join(cros_run_vm_test_cmd))

  return subprocess42.call(
      cros_run_vm_test_cmd, stdout=sys.stdout, stderr=sys.stderr, env=test_env)


def setup_env():
  """Returns a copy of the current env with some needed vars added."""
  env = os.environ.copy()
  # Some chromite scripts expect chromite/bin to be on PATH.
  env['PATH'] = env['PATH'] + ':' + os.path.join(CHROMITE_PATH, 'bin')
  # deploy_chrome needs a set of GN args used to build chrome to determine if
  # certain libraries need to be pushed to the VM. It looks for the args via
  # an env var. To trigger the default deploying behavior, give it a dummy set
  # of args.
  # TODO(crbug.com/823996): Make the GN-dependent deps controllable via cmd
  # line args.
  if not env.get('GN_ARGS'):
    env['GN_ARGS'] = 'enable_nacl = true'
  if not env.get('USE'):
    env['USE'] = 'highdpi'
  return env


def add_common_args(parser):
   parser.add_argument(
       '--cros-cache', type=str, default=DEFAULT_CROS_CACHE,
       help='Path to cros cache.')
   parser.add_argument(
       '--path-to-outdir', type=str, required=True,
       help='Path to output directory, all of whose contents will be '
            'deployed to the device.')
   parser.add_argument(
       '--runtime-deps-path', type=str,
       help='Runtime data dependency file from GN.')
   parser.add_argument(
       '--vpython-dir', type=str,
       help='Location on host of a directory containing a vpython binary to '
            'deploy to the VM before the test starts. The location of this '
            'dir will be added onto PATH in the VM. WARNING: The arch of the '
            'VM might not match the arch of the host, so avoid using '
            '"${platform}" when downloading vpython via CIPD.')
   parser.add_argument(
       '--vm-logs-dir', type=str,
       help='Will copy everything under /var/log/ from the VM after the test '
            'into the specified dir.')


def main():
  parser = argparse.ArgumentParser()
  parser.add_argument('--verbose', '-v', action='store_true')
  # Required args.
  parser.add_argument(
      '--board', type=str, required=True, help='Type of CrOS device.')
  subparsers = parser.add_subparsers(dest='test_type')
  # Host-side test args.
  host_cmd_parser = subparsers.add_parser(
      'host-cmd',
      help='Runs a host-side test. Pass the host-side command to run after '
           '"--". Hostname and port for the VM will be 127.0.0.1:9222.')
  host_cmd_parser.set_defaults(func=host_cmd)
  host_cmd_parser.add_argument(
      '--cros-cache', type=str, default=DEFAULT_CROS_CACHE,
      help='Path to cros cache.')
  host_cmd_parser.add_argument(
      '--path-to-outdir', type=os.path.realpath,
      help='Path to output directory, all of whose contents will be deployed '
           'to the device.')
  host_cmd_parser.add_argument(
      '--deploy-chrome', action='store_true',
      help='Will deploy a locally built Chrome binary to the VM before running '
           'the host-cmd.')
  host_cmd_parser.add_argument('cmd', nargs=argparse.REMAINDER)
  # GTest args.
  # TODO(bpastene): Rename 'vm-test' arg to 'gtest'.
  gtest_parser = subparsers.add_parser(
      'vm-test',
      help='Runs a vm-side gtest.')
  gtest_parser.set_defaults(func=vm_test)
  gtest_parser.add_argument(
      '--test-exe', type=str, required=True,
      help='Path to test executable to run inside VM. If the value is '
           '%s, the sanity test that ships with the VM '
           'image runs instead. This test smokes-check the system browser '
           '(eg: loads a simple webpage, executes some javascript), so a '
           'fully-built Chrome binary that can get deployed to the VM is '
           'expected to be available in the out-dir.' % SANITY_TEST_TARGET)

  # GTest args. Some are passed down to the test binary in the VM. Others are
  # parsed here since they might need tweaking or special handling.
  gtest_parser.add_argument(
      '--test-launcher-summary-output', type=str,
      help='When set, will pass the same option down to the test and retrieve '
           'its result file at the specified location.')
  # Shard args are parsed here since we might also specify them via env vars.
  gtest_parser.add_argument(
      '--test-launcher-shard-index',
      type=int, default=os.environ.get('GTEST_SHARD_INDEX', 0),
      help='Index of the external shard to run.')
  gtest_parser.add_argument(
      '--test-launcher-total-shards',
      type=int, default=os.environ.get('GTEST_TOTAL_SHARDS', 1),
      help='Total number of external shards.')

  # Tast test args.
  tast_test_parser = subparsers.add_parser(
      'tast',
      help='Runs a vm-side set of Tast tests.')
  tast_test_parser.set_defaults(func=vm_test)
  tast_test_parser.add_argument(
      '--suite-name', type=str, required=True,
      help='Name to apply to the set of Tast tests to run. This has no effect '
           'on what is executed, but is used mainly for test results reporting '
           'and tracking (eg: flakiness dashboard).')
  tast_test_parser.add_argument(
      '--test-launcher-summary-output', type=str,
      help='Generates a simple GTest-style JSON result file for the test run.')
  tast_test_parser.add_argument(
      '--conditional', type=str,
      help='A conditional whose matching tests will run '
           '(eg: ("dep:chrome" || "dep:chrome_login")).')
  tast_test_parser.add_argument(
      '--test', '-t', action='append', dest='tests',
      help='A Tast test to run in the VM (eg: "ui.ChromeLogin").')

  add_common_args(gtest_parser)
  add_common_args(tast_test_parser)
  args, unknown_args = parser.parse_known_args()

  logging.basicConfig(level=logging.DEBUG if args.verbose else logging.WARN)

  if not os.path.exists('/dev/kvm'):
    logging.error('/dev/kvm is missing. Is KVM installed on this machine?')
    return 1
  elif not os.access('/dev/kvm', os.W_OK):
    logging.error(
        '/dev/kvm is not writable as current user. Perhaps you should be root?')
    return 1

  args.cros_cache = os.path.abspath(args.cros_cache)
  return args.func(args, unknown_args)


if __name__ == '__main__':
  sys.exit(main())
