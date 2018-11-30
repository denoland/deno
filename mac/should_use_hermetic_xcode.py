#!/usr/bin/env python
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""
Prints "1" if Chrome targets should be built with hermetic Xcode.
Prints "2" if Chrome targets should be built with hermetic Xcode, but the OS
version does not meet the minimum requirements of the hermetic version of Xcode.
Otherwise prints "0".

Usage:
  python should_use_hermetic_xcode.py <target_os>
"""

import os
import sys

_THIS_DIR_PATH = os.path.abspath(os.path.dirname(os.path.realpath(__file__)))
_BUILD_PATH = os.path.join(_THIS_DIR_PATH, os.pardir)
sys.path.insert(0, _BUILD_PATH)

import mac_toolchain


def _IsCorpMachine():
  return os.path.isdir('/Library/GoogleCorpSupport/')


def main():
  allow_corp = sys.argv[1] == 'mac' and _IsCorpMachine()
  if os.environ.get('FORCE_MAC_TOOLCHAIN') or allow_corp:
    if not mac_toolchain.PlatformMeetsHermeticXcodeRequirements():
      return "2"
    return "1"
  else:
    return "0"


if __name__ == '__main__':
  print main()
  sys.exit(0)
