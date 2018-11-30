# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import devil_chromium
from pylib import constants
from pylib.base import environment


class LocalMachineEnvironment(environment.Environment):

  def __init__(self, _args, output_manager, _error_func):
    super(LocalMachineEnvironment, self).__init__(output_manager)

    devil_chromium.Initialize(
        output_directory=constants.GetOutDirectory())

  #override
  def SetUp(self):
    pass

  #override
  def TearDown(self):
    pass
