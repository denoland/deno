# Copyright (c) 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging
import time


class TimeProfile(object):
  """Class for simple profiling of action, with logging of cost."""

  def __init__(self, description='operation'):
    self._starttime = None
    self._endtime = None
    self._description = description
    self.Start()

  def Start(self):
    self._starttime = time.time()
    self._endtime = None

  def GetDelta(self):
    """Returns the rounded delta.

    Also stops the timer if Stop() has not already been called.
    """
    if self._endtime is None:
      self.Stop(log=False)
    delta = self._endtime - self._starttime
    delta = round(delta, 2) if delta < 10 else round(delta, 1)
    return delta

  def LogResult(self):
    """Logs the result."""
    logging.info('%s seconds to perform %s', self.GetDelta(), self._description)

  def Stop(self, log=True):
    """Stop profiling.

    Args:
      log: Log the delta (defaults to true).
    """
    self._endtime = time.time()
    if log:
      self.LogResult()
