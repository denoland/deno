# Copyright (c) 2012 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# pylint: disable=R0201

import glob
import logging
import os.path
import subprocess
import sys

from devil.android import device_errors
from devil.android.valgrind_tools import base_tool
from pylib.constants import DIR_SOURCE_ROOT


def SetChromeTimeoutScale(device, scale):
  """Sets the timeout scale in /data/local/tmp/chrome_timeout_scale to scale."""
  path = '/data/local/tmp/chrome_timeout_scale'
  if not scale or scale == 1.0:
    # Delete if scale is None/0.0/1.0 since the default timeout scale is 1.0
    device.RemovePath(path, force=True, as_root=True)
  else:
    device.WriteFile(path, '%f' % scale, as_root=True)



class AddressSanitizerTool(base_tool.BaseTool):
  """AddressSanitizer tool."""

  WRAPPER_NAME = '/system/bin/asanwrapper'
  # Disable memcmp overlap check.There are blobs (gl drivers)
  # on some android devices that use memcmp on overlapping regions,
  # nothing we can do about that.
  EXTRA_OPTIONS = 'strict_memcmp=0,use_sigaltstack=1'

  def __init__(self, device):
    super(AddressSanitizerTool, self).__init__()
    self._device = device

  @classmethod
  def CopyFiles(cls, device):
    """Copies ASan tools to the device."""
    libs = glob.glob(os.path.join(DIR_SOURCE_ROOT,
                                  'third_party/llvm-build/Release+Asserts/',
                                  'lib/clang/*/lib/linux/',
                                  'libclang_rt.asan-arm-android.so'))
    assert len(libs) == 1
    subprocess.call(
        [os.path.join(
             DIR_SOURCE_ROOT,
             'tools/android/asan/third_party/asan_device_setup.sh'),
         '--device', str(device),
         '--lib', libs[0],
         '--extra-options', AddressSanitizerTool.EXTRA_OPTIONS])
    device.WaitUntilFullyBooted()

  def GetTestWrapper(self):
    return AddressSanitizerTool.WRAPPER_NAME

  def GetUtilWrapper(self):
    """Returns the wrapper for utilities, such as forwarder.

    AddressSanitizer wrapper must be added to all instrumented binaries,
    including forwarder and the like. This can be removed if such binaries
    were built without instrumentation. """
    return self.GetTestWrapper()

  def SetupEnvironment(self):
    try:
      self._device.EnableRoot()
    except device_errors.CommandFailedError as e:
      # Try to set the timeout scale anyway.
      # TODO(jbudorick) Handle this exception appropriately after interface
      #                 conversions are finished.
      logging.error(str(e))
    SetChromeTimeoutScale(self._device, self.GetTimeoutScale())

  def CleanUpEnvironment(self):
    SetChromeTimeoutScale(self._device, None)

  def GetTimeoutScale(self):
    # Very slow startup.
    return 20.0


TOOL_REGISTRY = {
    'asan': AddressSanitizerTool,
}


def CreateTool(tool_name, device):
  """Creates a tool with the specified tool name.

  Args:
    tool_name: Name of the tool to create.
    device: A DeviceUtils instance.
  Returns:
    A tool for the specified tool_name.
  """
  if not tool_name:
    return base_tool.BaseTool()

  ctor = TOOL_REGISTRY.get(tool_name)
  if ctor:
    return ctor(device)
  else:
    print 'Unknown tool %s, available tools: %s' % (
        tool_name, ', '.join(sorted(TOOL_REGISTRY.keys())))
    sys.exit(1)

def PushFilesForTool(tool_name, device):
  """Pushes the files required for |tool_name| to |device|.

  Args:
    tool_name: Name of the tool to create.
    device: A DeviceUtils instance.
  """
  if not tool_name:
    return

  clazz = TOOL_REGISTRY.get(tool_name)
  if clazz:
    clazz.CopyFiles(device)
  else:
    print 'Unknown tool %s, available tools: %s' % (
        tool_name, ', '.join(sorted(TOOL_REGISTRY.keys())))
    sys.exit(1)
