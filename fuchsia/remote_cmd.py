# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Helper functions for remotely executing and copying files over a SSH
connection."""

import logging
import os
import subprocess
import sys

_SSH = ['ssh']
_SCP = ['scp', '-C']  # Use gzip compression.
_SSH_LOGGER = logging.getLogger('ssh')

COPY_TO_TARGET = 0
COPY_FROM_TARGET = 1


def _IsLinkLocalIPv6(hostname):
  return hostname.startswith('fe80::')


def RunSsh(config_path, host, port, command, silent):
  """Executes an SSH command on the remote host and blocks until completion.

  config_path: Full path to SSH configuration.
  host: The hostname or IP address of the remote host.
  port: The port to connect to.
  command: A list of strings containing the command and its arguments.
  silent: If true, suppresses all output from 'ssh'.

  Returns the exit code from the remote command."""

  ssh_command = _SSH + ['-F', config_path,
                        host,
                        '-p', str(port)] + command
  _SSH_LOGGER.debug('ssh exec: ' + ' '.join(ssh_command))
  if silent:
    devnull = open(os.devnull, 'w')
    return subprocess.call(ssh_command, stderr=devnull, stdout=devnull)
  else:
    return subprocess.call(ssh_command)


def RunPipedSsh(config_path, host, port, command = None, ssh_args = None,
                **kwargs):
  """Executes an SSH command on the remote host and returns a process object
  with access to the command's stdio streams. Does not block.

  config_path: Full path to SSH configuration.
  host: The hostname or IP address of the remote host.
  port: The port to connect to.
  command: A list of strings containing the command and its arguments.
  ssh_args: Arguments that will be passed to SSH.
  kwargs: A dictionary of parameters to be passed to subprocess.Popen().
          The parameters can be used to override stdin and stdout, for example.

  Returns a Popen object for the command."""

  if not command:
    command = []
  if not ssh_args:
    ssh_args = []

  ssh_command = _SSH + ['-F', config_path,
                        host,
                        '-p', str(port)] + ssh_args + ['--'] + command
  _SSH_LOGGER.debug(' '.join(ssh_command))
  return subprocess.Popen(ssh_command, **kwargs)


def RunScp(config_path, host, port, sources, dest, direction, recursive=False):
  """Copies a file to or from a remote host using SCP and blocks until
  completion.

  config_path: Full path to SSH configuration.
  host: The hostname or IP address of the remote host.
  port: The port to connect to.
  sources: Paths of the files to be copied.
  dest: The path that |source| will be copied to.
  direction: Indicates whether the file should be copied to
             or from the remote side.
             Valid values are COPY_TO_TARGET or COPY_FROM_TARGET.
  recursive: If true, performs a recursive copy.

  Function will raise an assertion if a failure occurred."""

  scp_command = _SCP[:]
  if ':' in host:
    scp_command.append('-6')
    host = '[' + host + ']'
  if _SSH_LOGGER.getEffectiveLevel() == logging.DEBUG:
    scp_command.append('-v')
  if recursive:
    scp_command.append('-r')

  if direction == COPY_TO_TARGET:
    dest = "%s:%s" % (host, dest)
  else:
    sources = ["%s:%s" % (host, source) for source in sources]

  scp_command += ['-F', config_path, '-P', str(port)]
  scp_command += sources
  scp_command += [dest]

  _SSH_LOGGER.debug(' '.join(scp_command))
  subprocess.check_call(scp_command, stdout=open(os.devnull, 'w'))
