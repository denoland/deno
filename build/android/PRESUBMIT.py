# Copyright (c) 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Presubmit script for android buildbot.

See http://dev.chromium.org/developers/how-tos/depottools/presubmit-scripts for
details on the presubmit API built into depot_tools.
"""


def CommonChecks(input_api, output_api):
  output = []

  build_android_dir = input_api.PresubmitLocalPath()

  def J(*dirs):
    """Returns a path relative to presubmit directory."""
    return input_api.os_path.join(build_android_dir, *dirs)

  build_pys = [
      r'gyp/.*\.py$',
      r'gn/.*\.py',
  ]
  output.extend(input_api.canned_checks.RunPylint(
      input_api,
      output_api,
      pylintrc='pylintrc',
      black_list=build_pys,
      extra_paths_list=[
          J(),
          J('gyp'),
          J('buildbot'),
          J('..', 'util', 'lib', 'common'),
          J('..', '..', 'third_party', 'catapult', 'common', 'py_trace_event'),
          J('..', '..', 'third_party', 'catapult', 'common', 'py_utils'),
          J('..', '..', 'third_party', 'catapult', 'devil'),
          J('..', '..', 'third_party', 'catapult', 'tracing'),
          J('..', '..', 'third_party', 'depot_tools'),
      ]))
  output.extend(input_api.canned_checks.RunPylint(
      input_api,
      output_api,
      white_list=build_pys,
      extra_paths_list=[J('gyp'), J('gn')]))

  # Disabled due to http://crbug.com/410936
  #output.extend(input_api.canned_checks.RunUnitTestsInDirectory(
  #input_api, output_api, J('buildbot', 'tests')))

  pylib_test_env = dict(input_api.environ)
  pylib_test_env.update({
      'PYTHONPATH': build_android_dir,
      'PYTHONDONTWRITEBYTECODE': '1',
  })
  output.extend(input_api.canned_checks.RunUnitTests(
      input_api,
      output_api,
      unit_tests=[
          J('.', 'emma_coverage_stats_test.py'),
          J('gyp', 'util', 'build_utils_test.py'),
          J('gyp', 'util', 'md5_check_test.py'),
          J('play_services', 'update_test.py'),
          J('pylib', 'constants', 'host_paths_unittest.py'),
          J('pylib', 'gtest', 'gtest_test_instance_test.py'),
          J('pylib', 'instrumentation',
            'instrumentation_test_instance_test.py'),
          J('pylib', 'local', 'device',
            'local_device_instrumentation_test_run_test.py'),
          J('pylib', 'local', 'device', 'local_device_test_run_test.py'),
          J('pylib', 'output', 'local_output_manager_test.py'),
          J('pylib', 'output', 'noop_output_manager_test.py'),
          J('pylib', 'output', 'remote_output_manager_test.py'),
          J('pylib', 'results', 'json_results_test.py'),
          J('pylib', 'symbols', 'apk_native_libs_unittest.py'),
          J('pylib', 'symbols', 'elf_symbolizer_unittest.py'),
          J('pylib', 'symbols', 'symbol_utils_unittest.py'),
          J('pylib', 'utils', 'decorators_test.py'),
          J('pylib', 'utils', 'device_dependencies_test.py'),
          J('pylib', 'utils', 'dexdump_test.py'),
          J('pylib', 'utils', 'proguard_test.py'),
          J('pylib', 'utils', 'test_filter_test.py'),
          J('.', 'convert_dex_profile_tests.py'),
      ],
      env=pylib_test_env))

  return output


def CheckChangeOnUpload(input_api, output_api):
  return CommonChecks(input_api, output_api)


def CheckChangeOnCommit(input_api, output_api):
  return CommonChecks(input_api, output_api)
