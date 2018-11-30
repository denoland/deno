# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

from pylib.gtest import gtest_test_instance
from pylib.instrumentation import instrumentation_test_instance
from pylib.junit import junit_test_instance
from pylib.linker import linker_test_instance
from pylib.monkey import monkey_test_instance
from pylib.perf import perf_test_instance
from pylib.utils import device_dependencies


def CreateTestInstance(args, error_func):

  if args.command == 'gtest':
    return gtest_test_instance.GtestTestInstance(
        args, device_dependencies.GetDataDependencies, error_func)
  elif args.command == 'instrumentation':
    return instrumentation_test_instance.InstrumentationTestInstance(
        args, device_dependencies.GetDataDependencies, error_func)
  elif args.command == 'junit':
    return junit_test_instance.JunitTestInstance(args, error_func)
  elif args.command == 'linker':
    return linker_test_instance.LinkerTestInstance(args)
  elif args.command == 'monkey':
    return monkey_test_instance.MonkeyTestInstance(args, error_func)
  elif args.command == 'perf':
    return perf_test_instance.PerfTestInstance(args, error_func)

  error_func('Unable to create %s test instance.' % args.command)
