# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
import platform
import sys

DIR_SOURCE_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), os.pardir, os.pardir))
SDK_ROOT = os.path.join(DIR_SOURCE_ROOT, 'third_party', 'fuchsia-sdk', 'sdk')

def EnsurePathExists(path):
  """Checks that the file |path| exists on the filesystem and returns the path
  if it does, raising an exception otherwise."""

  if not os.path.exists(path):
    raise IOError('Missing file: ' + path)

  return path

def GetHostOsFromPlatform():
  host_platform = sys.platform
  if host_platform.startswith('linux'):
    return 'linux'
  elif host_platform.startswith('darwin'):
    return 'mac'
  raise Exception('Unsupported host platform: %s' % host_platform)

def GetHostArchFromPlatform():
  host_arch = platform.machine()
  if host_arch == 'x86_64':
    return 'x64'
  elif host_arch == 'aarch64':
    return 'arm64'
  raise Exception('Unsupported host architecture: %s' % host_arch)

def GetQemuRootForPlatform():
  return os.path.join(DIR_SOURCE_ROOT, 'third_party',
                      'qemu-' + GetHostOsFromPlatform() + '-' +
                       GetHostArchFromPlatform())
