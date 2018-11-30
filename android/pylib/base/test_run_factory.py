# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

from pylib.gtest import gtest_test_instance
from pylib.instrumentation import instrumentation_test_instance
from pylib.junit import junit_test_instance
from pylib.linker import linker_test_instance
from pylib.monkey import monkey_test_instance
from pylib.local.device import local_device_environment
from pylib.local.device import local_device_gtest_run
from pylib.local.device import local_device_instrumentation_test_run
from pylib.local.device import local_device_linker_test_run
from pylib.local.device import local_device_monkey_test_run
from pylib.local.device import local_device_perf_test_run
from pylib.local.machine import local_machine_environment
from pylib.local.machine import local_machine_junit_test_run
from pylib.perf import perf_test_instance


def _CreatePerfTestRun(args, env, test_instance):
  if args.print_step:
    return local_device_perf_test_run.PrintStep(
        env, test_instance)
  elif args.output_json_list:
    return local_device_perf_test_run.OutputJsonList(
        env, test_instance)
  return local_device_perf_test_run.LocalDevicePerfTestRun(
      env, test_instance)


def CreateTestRun(args, env, test_instance, error_func):
  if isinstance(env, local_device_environment.LocalDeviceEnvironment):
    if isinstance(test_instance, gtest_test_instance.GtestTestInstance):
      return local_device_gtest_run.LocalDeviceGtestRun(env, test_instance)
    if isinstance(test_instance,
                  instrumentation_test_instance.InstrumentationTestInstance):
      return (local_device_instrumentation_test_run
              .LocalDeviceInstrumentationTestRun(env, test_instance))
    if isinstance(test_instance, linker_test_instance.LinkerTestInstance):
      return (local_device_linker_test_run
              .LocalDeviceLinkerTestRun(env, test_instance))
    if isinstance(test_instance, monkey_test_instance.MonkeyTestInstance):
      return (local_device_monkey_test_run
              .LocalDeviceMonkeyTestRun(env, test_instance))
    if isinstance(test_instance,
                  perf_test_instance.PerfTestInstance):
      return _CreatePerfTestRun(args, env, test_instance)

  if isinstance(env, local_machine_environment.LocalMachineEnvironment):
    if isinstance(test_instance, junit_test_instance.JunitTestInstance):
      return (local_machine_junit_test_run
              .LocalMachineJunitTestRun(env, test_instance))

  error_func('Unable to create test run for %s tests in %s environment'
             % (str(test_instance), str(env)))
