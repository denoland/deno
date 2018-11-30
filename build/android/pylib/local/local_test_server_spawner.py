# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import json
import time

from devil.android import forwarder
from devil.android import ports
from pylib.base import test_server
from pylib.constants import host_paths

with host_paths.SysPath(host_paths.BUILD_COMMON_PATH):
  import chrome_test_server_spawner


# The tests should not need more than one test server instance.
MAX_TEST_SERVER_INSTANCES = 1


def _WaitUntil(predicate, max_attempts=5):
  """Blocks until the provided predicate (function) is true.

  Returns:
    Whether the provided predicate was satisfied once (before the timeout).
  """
  sleep_time_sec = 0.025
  for _ in xrange(1, max_attempts):
    if predicate():
      return True
    time.sleep(sleep_time_sec)
    sleep_time_sec = min(1, sleep_time_sec * 2)  # Don't wait more than 1 sec.
  return False


class PortForwarderAndroid(chrome_test_server_spawner.PortForwarder):
  def __init__(self, device, tool):
    self.device = device
    self.tool = tool

  def Map(self, port_pairs):
    forwarder.Forwarder.Map(port_pairs, self.device, self.tool)

  def GetDevicePortForHostPort(self, host_port):
    return forwarder.Forwarder.DevicePortForHostPort(host_port)

  def WaitHostPortAvailable(self, port):
    return _WaitUntil(lambda: ports.IsHostPortAvailable(port))

  def WaitPortNotAvailable(self, port):
    return _WaitUntil(lambda: not ports.IsHostPortAvailable(port))

  def WaitDevicePortReady(self, port):
    return _WaitUntil(lambda: ports.IsDevicePortUsed(self.device, port))

  def Unmap(self, device_port):
    forwarder.Forwarder.UnmapDevicePort(device_port, self.device)


class LocalTestServerSpawner(test_server.TestServer):

  def __init__(self, port, device, tool):
    super(LocalTestServerSpawner, self).__init__()
    self._device = device
    self._spawning_server = chrome_test_server_spawner.SpawningServer(
        port, PortForwarderAndroid(device, tool), MAX_TEST_SERVER_INSTANCES)
    self._tool = tool

  @property
  def server_address(self):
    return self._spawning_server.server.server_address

  @property
  def port(self):
    return self.server_address[1]

  #override
  def SetUp(self):
    # See net/test/spawned_test_server/test_server_config.h for description of
    # the fields in the config file.
    test_server_config = json.dumps({
      'address': '127.0.0.1',
      'spawner_url_base': 'http://localhost:%d' % self.port
    })
    self._device.WriteFile(
        '%s/net-test-server-config' % self._device.GetExternalStoragePath(),
        test_server_config)
    forwarder.Forwarder.Map(
        [(self.port, self.port)], self._device, self._tool)
    self._spawning_server.Start()

  #override
  def Reset(self):
    self._spawning_server.CleanupState()

  #override
  def TearDown(self):
    self.Reset()
    self._spawning_server.Stop()
    forwarder.Forwarder.UnmapDevicePort(self.port, self._device)
