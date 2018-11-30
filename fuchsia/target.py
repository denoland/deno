# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import boot_data
import common
import logging
import remote_cmd
import sys
import time


_SHUTDOWN_CMD = ['dm', 'poweroff']
_ATTACH_MAX_RETRIES = 10
_ATTACH_RETRY_INTERVAL = 1


class FuchsiaTargetException(Exception):
  def __init__(self, message):
    super(FuchsiaTargetException, self).__init__(message)


class Target(object):
  """Base class representing a Fuchsia deployment target."""

  def __init__(self, output_dir, target_cpu):
    self._output_dir = output_dir
    self._started = False
    self._dry_run = False
    self._target_cpu = target_cpu

  # Functions used by the Python context manager for teardown.
  def __enter__(self):
    return self
  def __exit__(self, exc_type, exc_val, exc_tb):
    return self

  def Start(self):
    """Handles the instantiation and connection process for the Fuchsia
    target instance."""

    pass

  def IsStarted(self):
    """Returns True if the Fuchsia target instance is ready to accept
    commands."""

    return self._started

  def IsNewInstance(self):
    """Returns True if the connected target instance is newly provisioned."""

    return True

  def RunCommandPiped(self, command, **kwargs):
    """Starts a remote command and immediately returns a Popen object for the
    command. The caller may interact with the streams, inspect the status code,
    wait on command termination, etc.

    command: A list of strings representing the command and arguments.
    kwargs: A dictionary of parameters to be passed to subprocess.Popen().
            The parameters can be used to override stdin and stdout, for
            example.

    Returns: a Popen object.

    Note: method does not block."""

    self._AssertIsStarted()
    logging.debug('running (non-blocking) \'%s\'.' % ' '.join(command))
    host, port = self._GetEndpoint()
    return remote_cmd.RunPipedSsh(self._GetSshConfigPath(), host, port, command,
                                  **kwargs)

  def RunCommand(self, command, silent=False):
    """Executes a remote command and waits for it to finish executing.

    Returns the exit code of the command."""

    self._AssertIsStarted()
    logging.debug('running \'%s\'.' % ' '.join(command))
    host, port = self._GetEndpoint()
    return remote_cmd.RunSsh(self._GetSshConfigPath(), host, port, command,
                             silent)

  def PutFile(self, source, dest, recursive=False):
    """Copies a file from the local filesystem to the target filesystem.

    source: The path of the file being copied.
    dest: The path on the remote filesystem which will be copied to.
    recursive: If true, performs a recursive copy."""

    assert type(source) is str
    self.PutFiles([source], dest, recursive)

  def PutFiles(self, sources, dest, recursive=False):
    """Copies files from the local filesystem to the target filesystem.

    sources: List of local file paths to copy from, or a single path.
    dest: The path on the remote filesystem which will be copied to.
    recursive: If true, performs a recursive copy."""

    assert type(sources) is tuple or type(sources) is list
    self._AssertIsStarted()
    host, port = self._GetEndpoint()
    logging.debug('copy local:%s => remote:%s' % (sources, dest))
    command = remote_cmd.RunScp(self._GetSshConfigPath(), host, port,
                                sources, dest, remote_cmd.COPY_TO_TARGET,
                                recursive)

  def GetFile(self, source, dest):
    """Copies a file from the target filesystem to the local filesystem.

    source: The path of the file being copied.
    dest: The path on the local filesystem which will be copied to."""
    assert type(source) is str
    self.GetFiles([source], dest)

  def GetFiles(self, sources, dest):
    """Copies files from the target filesystem to the local filesystem.

    sources: List of remote file paths to copy.
    dest: The path on the local filesystem which will be copied to."""
    assert type(sources) is tuple or type(sources) is list
    self._AssertIsStarted()
    host, port = self._GetEndpoint()
    logging.debug('copy remote:%s => local:%s' % (sources, dest))
    return remote_cmd.RunScp(self._GetSshConfigPath(), host, port,
                             sources, dest, remote_cmd.COPY_FROM_TARGET)

  def _GetEndpoint(self):
    """Returns a (host, port) tuple for the SSH connection to the target."""
    raise NotImplementedError

  def _GetTargetSdkArch(self):
    """Returns the Fuchsia SDK architecture name for the target CPU."""
    if self._target_cpu == 'arm64' or self._target_cpu == 'x64':
      return self._target_cpu
    raise FuchsiaTargetException('Unknown target_cpu:' + self._target_cpu)

  def _AssertIsStarted(self):
    assert self.IsStarted()

  def _WaitUntilReady(self, retries=_ATTACH_MAX_RETRIES):
    logging.info('Connecting to Fuchsia using SSH.')

    for retry in xrange(retries + 1):
      host, port = self._GetEndpoint()
      if remote_cmd.RunSsh(self._GetSshConfigPath(), host, port, ['true'],
                           True) == 0:
        logging.info('Connected!')
        self._started = True
        return True
      time.sleep(_ATTACH_RETRY_INTERVAL)

    logging.error('Timeout limit reached.')

    raise FuchsiaTargetException('Couldn\'t connect using SSH.')

  def _GetSshConfigPath(self, path):
    raise NotImplementedError

  # TODO: remove this once all instances of architecture names have been
  # converted to the new naming pattern.
  def _GetTargetSdkLegacyArch(self):
    """Returns the Fuchsia SDK architecture name for the target CPU."""
    if self._target_cpu == 'arm64':
      return 'aarch64'
    elif self._target_cpu == 'x64':
      return 'x86_64'
    raise Exception('Unknown target_cpu %s:' % self._target_cpu)
