# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import random

from pylib import constants
from pylib.base import test_instance


_SINGLE_EVENT_TIMEOUT = 100 # Milliseconds

class MonkeyTestInstance(test_instance.TestInstance):

  def __init__(self, args, _):
    super(MonkeyTestInstance, self).__init__()

    self._categories = args.categories
    self._event_count = args.event_count
    self._seed = args.seed or random.randint(1, 100)
    self._throttle = args.throttle
    self._verbose_count = args.verbose_count

    self._package = constants.PACKAGE_INFO[args.browser].package
    self._activity = constants.PACKAGE_INFO[args.browser].activity

    self._timeout_s = (
        self.event_count * (self.throttle + _SINGLE_EVENT_TIMEOUT)) / 1000

  #override
  def TestType(self):
    return 'monkey'

  #override
  def SetUp(self):
    pass

  #override
  def TearDown(self):
    pass

  @property
  def activity(self):
    return self._activity

  @property
  def categories(self):
    return self._categories

  @property
  def event_count(self):
    return self._event_count

  @property
  def package(self):
    return self._package

  @property
  def seed(self):
    return self._seed

  @property
  def throttle(self):
    return self._throttle

  @property
  def timeout(self):
    return self._timeout_s

  @property
  def verbose_count(self):
    return self._verbose_count
