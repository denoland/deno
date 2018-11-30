# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

from pylib.base import environment
from pylib.constants import host_paths

with host_paths.SysPath(host_paths.PYMOCK_PATH):
  import mock  # pylint: disable=import-error


MockEnvironment = mock.MagicMock(environment.Environment)
