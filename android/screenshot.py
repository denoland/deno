#!/usr/bin/env python
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import sys

import devil_chromium
from devil.android.tools import screenshot

if __name__ == '__main__':
  devil_chromium.Initialize()
  sys.exit(screenshot.main())
