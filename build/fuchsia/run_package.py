# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Contains a helper function for deploying and executing a packaged
executable on a Target."""

import common
import json
import logging
import multiprocessing
import os
import select
import shutil
import subprocess
import sys
import tempfile
import threading
import uuid

from symbolizer import FilterStream

FAR = os.path.join(common.SDK_ROOT, 'tools', 'far')
PM = os.path.join(common.SDK_ROOT, 'tools', 'pm')

# Amount of time to wait for the termination of the system log output thread.
_JOIN_TIMEOUT_SECS = 5


def _AttachKernelLogReader(target):
  """Attaches a kernel log reader as a long-running SSH task."""

  logging.info('Attaching kernel logger.')
  return target.RunCommandPiped(['dlog', '-f'], stdin=open(os.devnull, 'r'),
                                stdout=subprocess.PIPE)


def _ReadMergedLines(streams):
  """Creates a generator which merges the buffered line output from |streams|.
  The generator is terminated when the primary (first in sequence) stream
  signals EOF. Absolute output ordering is not guaranteed."""

  assert len(streams) > 0
  streams_by_fd = {}
  primary_fd = streams[0].fileno()
  for s in streams:
    streams_by_fd[s.fileno()] = s

  while primary_fd != None:
    rlist, _, _ = select.select(streams_by_fd, [], [], 0.1)
    for fileno in rlist:
      line = streams_by_fd[fileno].readline()
      if line:
        yield line
      elif fileno == primary_fd:
        primary_fd = None
      else:
        del streams_by_fd[fileno]


def DrainStreamToStdout(stream, quit_event):
  """Outputs the contents of |stream| until |quit_event| is set."""

  while not quit_event.is_set():
    rlist, _, _ = select.select([ stream ], [], [], 0.1)
    if rlist:
      line = rlist[0].readline()
      if not line:
        return
      print line.rstrip()


def RunPackage(output_dir, target, package_path, package_name, package_deps,
               run_args, system_logging, install_only, symbolizer_config=None):
  """Copies the Fuchsia package at |package_path| to the target,
  executes it with |run_args|, and symbolizes its output.

  output_dir: The path containing the build output files.
  target: The deployment Target object that will run the package.
  package_path: The path to the .far package file.
  package_name: The name of app specified by package metadata.
  run_args: The arguments which will be passed to the Fuchsia process.
  system_logging: If set, connects a system log reader to the target.
  install_only: If set, skips the package execution step.
  symbolizer_config: A newline delimited list of source files contained
                     in the package. Omitting this parameter will disable
                     symbolization.

  Returns the exit code of the remote package process."""


  system_logger = _AttachKernelLogReader(target) if system_logging else None
  try:
    if system_logger:
      # Spin up a thread to asynchronously dump the system log to stdout
      # for easier diagnoses of early, pre-execution failures.
      log_output_quit_event = multiprocessing.Event()
      log_output_thread = threading.Thread(
          target=lambda: DrainStreamToStdout(system_logger.stdout,
                                             log_output_quit_event))
      log_output_thread.daemon = True
      log_output_thread.start()

    for next_package_path in ([package_path] + package_deps):
      logging.info('Installing ' + os.path.basename(next_package_path) + '.')

      # Copy the package archive.
      install_path = os.path.join('/data', os.path.basename(next_package_path))
      target.PutFile(next_package_path, install_path)

      # Install the package.
      p = target.RunCommandPiped(['pm', 'install', install_path],
                                 stderr=subprocess.PIPE)
      output = p.stderr.readlines()
      p.wait()
      if p.returncode != 0:
        # Don't error out if the package already exists on the device.
        if len(output) != 1 or 'ErrAlreadyExists' not in output[0]:
          raise Exception('Error while installing: %s' % '\n'.join(output))

      # Clean up the package archive.
      target.RunCommand(['rm', install_path])

    if system_logger:
      log_output_quit_event.set()
      log_output_thread.join(timeout=_JOIN_TIMEOUT_SECS)

    if install_only:
      logging.info('Installation complete.')
      return

    logging.info('Running application.')
    command = ['run', package_name] + run_args
    process = target.RunCommandPiped(command,
                                     stdin=open(os.devnull, 'r'),
                                     stdout=subprocess.PIPE,
                                     stderr=subprocess.STDOUT)

    if system_logger:
      task_output = _ReadMergedLines([process.stdout, system_logger.stdout])
    else:
      task_output = process.stdout

    if symbolizer_config:
      # Decorate the process output stream with the symbolizer.
      output = FilterStream(task_output, package_name, symbolizer_config,
                            output_dir)
    else:
      logging.warn('Symbolization is DISABLED.')
      output = process.stdout

    for next_line in output:
      print next_line.rstrip()

    process.wait()
    if process.returncode == 0:
      logging.info('Process exited normally with status code 0.')
    else:
      # The test runner returns an error status code if *any* tests fail,
      # so we should proceed anyway.
      logging.warning('Process exited with status code %d.' %
                      process.returncode)

  finally:
    if system_logger:
      logging.info('Terminating kernel log reader.')
      log_output_quit_event.set()
      log_output_thread.join()
      system_logger.kill()


  return process.returncode
