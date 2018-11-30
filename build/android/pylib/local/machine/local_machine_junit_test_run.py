# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import json
import logging
import os
import zipfile

from devil.utils import cmd_helper
from devil.utils import reraiser_thread
from pylib import constants
from pylib.base import base_test_result
from pylib.base import test_run
from pylib.results import json_results
from py_utils import tempfile_ext


class LocalMachineJunitTestRun(test_run.TestRun):
  def __init__(self, env, test_instance):
    super(LocalMachineJunitTestRun, self).__init__(env, test_instance)

  #override
  def TestPackage(self):
    return self._test_instance.suite

  #override
  def SetUp(self):
    pass

  #override
  def RunTests(self):
    with tempfile_ext.NamedTemporaryDirectory() as temp_dir:
      json_file_path = os.path.join(temp_dir, 'results.json')

      # Extract resources needed for test.
      # TODO(mikecase): Investigate saving md5sums of zipfiles, and only
      # extract zipfiles when they change.
      def extract_resource_zip(resource_zip):
        def helper():
          extract_dest = os.path.join(
              temp_dir, os.path.splitext(os.path.basename(resource_zip))[0])
          with zipfile.ZipFile(resource_zip, 'r') as zf:
            zf.extractall(extract_dest)
          return extract_dest
        return helper

      resource_dirs = reraiser_thread.RunAsync(
          [extract_resource_zip(resource_zip)
           for resource_zip in self._test_instance.resource_zips
           if os.path.exists(resource_zip)])

      java_script = os.path.join(
          constants.GetOutDirectory(), 'bin', 'helper',
          self._test_instance.suite)
      command = [java_script]

      # Add Jar arguments.
      jar_args = ['-test-jars', self._test_instance.suite + '.jar',
                  '-json-results-file', json_file_path]
      if self._test_instance.test_filter:
        jar_args.extend(['-gtest-filter', self._test_instance.test_filter])
      if self._test_instance.package_filter:
        jar_args.extend(['-package-filter',
                         self._test_instance.package_filter])
      if self._test_instance.runner_filter:
        jar_args.extend(['-runner-filter', self._test_instance.runner_filter])
      command.extend(['--jar-args', '"%s"' % ' '.join(jar_args)])

      # Add JVM arguments.
      jvm_args = ['-Drobolectric.dependency.dir=%s' %
                      self._test_instance.robolectric_runtime_deps_dir,
                  '-Ddir.source.root=%s' % constants.DIR_SOURCE_ROOT,]

      if self._test_instance.android_manifest_path:
        jvm_args += ['-Dchromium.robolectric.manifest=%s' %
                     self._test_instance.android_manifest_path]

      if self._test_instance.package_name:
        jvm_args += ['-Dchromium.robolectric.package.name=%s' %
                     self._test_instance.package_name]

      if resource_dirs:
        jvm_args += ['-Dchromium.robolectric.resource.dirs=%s' %
                     ':'.join(resource_dirs)]

      if logging.getLogger().isEnabledFor(logging.INFO):
        jvm_args += ['-Drobolectric.logging=stdout']

      if self._test_instance.debug_socket:
        jvm_args += ['-agentlib:jdwp=transport=dt_socket'
                     ',server=y,suspend=y,address=%s' %
                     self._test_instance.debug_socket]

      if self._test_instance.coverage_dir:
        if not os.path.exists(self._test_instance.coverage_dir):
          os.makedirs(self._test_instance.coverage_dir)
        elif not os.path.isdir(self._test_instance.coverage_dir):
          raise Exception('--coverage-dir takes a directory, not file path.')
        jvm_args.append('-Demma.coverage.out.file=%s' % os.path.join(
            self._test_instance.coverage_dir,
            '%s.ec' % self._test_instance.suite))

      if jvm_args:
        command.extend(['--jvm-args', '"%s"' % ' '.join(jvm_args)])

      cmd_helper.RunCmd(command)
      try:
        with open(json_file_path, 'r') as f:
          results_list = json_results.ParseResultsFromJson(
              json.loads(f.read()))
      except IOError:
        # In the case of a failure in the JUnit or Robolectric test runner
        # the output json file may never be written.
        results_list = [
          base_test_result.BaseTestResult(
              'Test Runner Failure', base_test_result.ResultType.UNKNOWN)
        ]

      test_run_results = base_test_result.TestRunResults()
      test_run_results.AddResults(results_list)

      return [test_run_results]

  #override
  def TearDown(self):
    pass
