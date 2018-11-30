# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import json
import logging
import os
import re
import select
import socket
import sys
import subprocess
import tempfile
import time

DIR_SOURCE_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), os.pardir, os.pardir))
sys.path.append(os.path.join(DIR_SOURCE_ROOT, 'build', 'util', 'lib', 'common'))
import chrome_test_server_spawner

PORT_MAP_RE = re.compile('Allocated port (?P<port>\d+) for remote')
GET_PORT_NUM_TIMEOUT_SECS = 5


def _ConnectPortForwardingTask(target, local_port):
  """Establishes a port forwarding SSH task to a localhost TCP endpoint hosted
  at port |local_port|. Blocks until port forwarding is established.

  Returns the remote port number."""

  forwarding_flags = ['-O', 'forward',  # Send SSH mux control signal.
                      '-R', '0:localhost:%d' % local_port,
                      '-v',   # Get forwarded port info from stderr.
                      '-NT']  # Don't execute command; don't allocate terminal.
  task = target.RunCommandPiped([],
                                ssh_args=forwarding_flags,
                                stderr=subprocess.PIPE)

  # SSH reports the remote dynamic port number over stderr.
  # Unfortunately, the output is incompatible with Python's line buffered
  # input (or vice versa), so we have to build our own buffered input system to
  # pull bytes over the pipe.
  poll_obj = select.poll()
  poll_obj.register(task.stderr, select.POLLIN)
  line = ''
  timeout = time.time() + GET_PORT_NUM_TIMEOUT_SECS
  while time.time() < timeout:
    poll_result = poll_obj.poll(max(0, timeout - time.time()))
    if poll_result:
      next_char = task.stderr.read(1)
      if not next_char:
        break
      line += next_char
      if line.endswith('\n'):
        line = line[:-1]
        logging.debug('ssh: ' + line)
        matched = PORT_MAP_RE.match(line)
        if matched:
          device_port = int(matched.group('port'))
          logging.debug('Port forwarding established (local=%d, device=%d)' %
                        (local_port, device_port))
          task.wait()
          return device_port
        line = ''

  raise Exception('Could not establish a port forwarding connection.')


# Implementation of chrome_test_server_spawner.PortForwarder that uses SSH's
# remote port forwarding feature to forward ports.
class SSHPortForwarder(chrome_test_server_spawner.PortForwarder):
  def __init__(self, target):
    self._target = target

    # Maps the host (server) port to the device port number.
    self._port_mapping = {}

  def Map(self, port_pairs):
    for p in port_pairs:
      _, host_port = p
      self._port_mapping[host_port] = \
          _ConnectPortForwardingTask(self._target, host_port)

  def GetDevicePortForHostPort(self, host_port):
    return self._port_mapping[host_port]

  def Unmap(self, device_port):
    for host_port, entry in self._port_mapping.iteritems():
      if entry == device_port:
        forwarding_args = [
            '-NT', '-O', 'cancel', '-R',
            '%d:localhost:%d' % (self._port_mapping[host_port], host_port)]
        task = self._target.RunCommandPiped([],
                                            ssh_args=forwarding_args,
                                            stderr=subprocess.PIPE)
        task.wait()
        if task.returncode != 0:
          raise Exception(
              'Error %d when unmapping port %d' % (task.returncode,
                                                   device_port))
        del self._port_mapping[host_port]
        return

    raise Exception('Unmap called for unknown port: %d' % device_port)


def SetupTestServer(target, test_concurrency):
  """Provisions a forwarding test server and configures |target| to use it.

  Returns a Popen object for the test server process."""

  logging.debug('Starting test server.')
  spawning_server = chrome_test_server_spawner.SpawningServer(
      0, SSHPortForwarder(target), test_concurrency)
  forwarded_port = _ConnectPortForwardingTask(
      target, spawning_server.server_port)
  spawning_server.Start()

  logging.debug('Test server listening for connections (port=%d)' %
                spawning_server.server_port)
  logging.debug('Forwarded port is %d' % forwarded_port)

  config_file = tempfile.NamedTemporaryFile(delete=True)

  # Clean up the config JSON to only pass ports. See https://crbug.com/810209 .
  config_file.write(json.dumps({
    'name': 'testserver',
    'address': '127.0.0.1',
    'spawner_url_base': 'http://localhost:%d' % forwarded_port
  }))

  config_file.flush()
  target.RunCommand(['mkdir /data/shared'])
  target.PutFile(config_file.name, '/data/shared/net-test-server-config')

  return spawning_server
