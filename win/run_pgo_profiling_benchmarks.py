# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Utility script to run the benchmarks during the profiling step of a PGO
build.
"""

import json
import optparse
import os
import subprocess
import sys

# Make sure that we're running as admin, this is required to run the Telemetry
# benchmarks.
from win32com.shell import shell
if not shell.IsUserAnAdmin():
  raise Exception('This script has to be run as admin.')


_SCRIPT_DIR = os.path.dirname(os.path.realpath(__file__))
_CHROME_BUILD_DIR = os.path.dirname(_SCRIPT_DIR)
_CHROME_SRC_DIR = os.path.dirname(_CHROME_BUILD_DIR)


# List of the benchmark that we run during the profiling step.
_BENCHMARKS_TO_RUN = {
  'blink_perf.bindings',
  'blink_perf.canvas',
  'blink_perf.css',
  'blink_perf.dom',
  'blink_perf.paint',
  'blink_perf.svg',
  'blink_style.top_25',
  'dromaeo.cssqueryjquery',
  'dromaeo.domcoreattr',
  'dromaeo.domcoremodify',
  'dromaeo.domcorequery',
  'dromaeo.domcoretraverse',
  'dromaeo.jslibattrprototype',
  'dromaeo.jslibeventprototype',
  'dromaeo.jslibmodifyprototype',
  'dromaeo.jslibstyleprototype',
  'dromaeo.jslibtraversejquery',
  'dromaeo.jslibtraverseprototype',
  'media.tough_video_cases',
  'octane',
  'smoothness.top_25_smooth',
  'storage.indexeddb_endure_tracing',
  'sunspider',
}


def RunBenchmarks(options):
  """Run the benchmarks."""
  # Find the run_benchmark script.
  chrome_run_benchmark_script = os.path.join(_CHROME_SRC_DIR, 'tools',
                                             'perf', 'run_benchmark')
  if not os.path.exists(chrome_run_benchmark_script):
    raise Exception('Unable to find the run_benchmark script '
                    '(%s doesn\'t exist) ' % chrome_run_benchmark_script)

  # Augment the PATH to make sure that the benchmarking script can find
  # pgosweep.exe and its runtime libraries.
  env = os.environ.copy()
  env['PATH'] = str(os.pathsep.join([options.build_dir, os.environ['PATH']]))
  env['PogoSafeMode'] = '1'
  # Apply a scaling factor of 0.5 to the PGO profiling buffers for the 32-bit
  # builds, without this the buffers will be too large and the process will
  # fail to start. See crbug.com/632864#c22.
  if options.target_cpu == 'x86':
    env['VCPROFILE_ALLOC_SCALE'] = '0.5'

  # Run all the benchmarks.
  # TODO(sebmarchand): Make this run in parallel.
  for benchmark in _BENCHMARKS_TO_RUN:
    try:
      benchmark_command = [
          sys.executable,
          chrome_run_benchmark_script,
          '--browser', options.browser_type,
        ]
      # Automatically set the arguments to run this script on a local build.
      if options.browser_type == 'exact':
        benchmark_command += [
          '--browser-executable', os.path.join(options.build_dir, 'chrome.exe')
        ]
      benchmark_command += [
          '--profiler', 'win_pgo_profiler',
          benchmark
        ]
      subprocess.check_call(benchmark_command, env=env)
    except:
      print ('Error while trying to run the %s benchmark, continuing.' %
             benchmark)
      continue

  return 0


def main():
  parser = optparse.OptionParser(usage='%prog [options]')
  parser.add_option(
      '--browser-type', help='The browser type (to be passed to Telemetry\'s '
                              'benchmark runner).')
  # TODO(sebmarchand): Parse the args.gn file to automatically set this value.
  parser.add_option('--target-cpu', help='The target\'s bitness.')
  parser.add_option('--build-dir', help='Chrome build directory.')
  options, _ = parser.parse_args()

  if not options.target_cpu:
    parser.error('--target-cpu is required')
  if not options.build_dir:
    parser.error('--build-dir is required')
  if not options.browser_type:
    options.browser_type = 'exact'

  return RunBenchmarks(options)


if __name__ == '__main__':
  sys.exit(main())
