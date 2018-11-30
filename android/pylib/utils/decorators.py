# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import functools
import logging


def Memoize(f):
  """Decorator to cache return values of function."""
  memoize_dict = {}
  @functools.wraps(f)
  def wrapper(*args, **kwargs):
    key = repr((args, kwargs))
    if key not in memoize_dict:
      memoize_dict[key] = f(*args, **kwargs)
    return memoize_dict[key]
  return wrapper


def NoRaiseException(default_return_value=None, exception_message=''):
  """Returns decorator that catches and logs uncaught Exceptions.

  Args:
    default_return_value: Value to return in the case of uncaught Exception.
    exception_message: Message for uncaught exceptions.
  """
  def decorator(f):
    @functools.wraps(f)
    def wrapper(*args, **kwargs):
      try:
        return f(*args, **kwargs)
      except Exception:  # pylint: disable=broad-except
        logging.exception(exception_message)
        return default_return_value
    return wrapper
  return decorator
