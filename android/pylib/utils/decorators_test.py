#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Unit tests for decorators.py."""

import unittest

from pylib.utils import decorators


class NoRaiseExceptionDecoratorTest(unittest.TestCase):

  def testFunctionDoesNotRaiseException(self):
    """Tests that the |NoRaiseException| decorator catches exception."""

    @decorators.NoRaiseException()
    def raiseException():
      raise Exception()

    try:
      raiseException()
    except Exception:  # pylint: disable=broad-except
      self.fail('Exception was not caught by |NoRaiseException| decorator')

  def testFunctionReturnsCorrectValues(self):
    """Tests that the |NoRaiseException| decorator returns correct values."""

    @decorators.NoRaiseException(default_return_value=111)
    def raiseException():
      raise Exception()

    @decorators.NoRaiseException(default_return_value=111)
    def doesNotRaiseException():
      return 999

    self.assertEquals(raiseException(), 111)
    self.assertEquals(doesNotRaiseException(), 999)


class MemoizeDecoratorTest(unittest.TestCase):

  def testFunctionExceptionNotMemoized(self):
    """Tests that |Memoize| decorator does not cache exception results."""

    class ExceptionType1(Exception):
      pass

    class ExceptionType2(Exception):
      pass

    @decorators.Memoize
    def raiseExceptions():
      if raiseExceptions.count == 0:
        raiseExceptions.count += 1
        raise ExceptionType1()

      if raiseExceptions.count == 1:
        raise ExceptionType2()
    raiseExceptions.count = 0

    with self.assertRaises(ExceptionType1):
      raiseExceptions()
    with self.assertRaises(ExceptionType2):
      raiseExceptions()

  def testFunctionResultMemoized(self):
    """Tests that |Memoize| decorator caches results."""

    @decorators.Memoize
    def memoized():
      memoized.count += 1
      return memoized.count
    memoized.count = 0

    def notMemoized():
      notMemoized.count += 1
      return notMemoized.count
    notMemoized.count = 0

    self.assertEquals(memoized(), 1)
    self.assertEquals(memoized(), 1)
    self.assertEquals(memoized(), 1)

    self.assertEquals(notMemoized(), 1)
    self.assertEquals(notMemoized(), 2)
    self.assertEquals(notMemoized(), 3)

  def testFunctionMemoizedBasedOnArgs(self):
    """Tests that |Memoize| caches results based on args and kwargs."""

    @decorators.Memoize
    def returnValueBasedOnArgsKwargs(a, k=0):
      return a + k

    self.assertEquals(returnValueBasedOnArgsKwargs(1, 1), 2)
    self.assertEquals(returnValueBasedOnArgsKwargs(1, 2), 3)
    self.assertEquals(returnValueBasedOnArgsKwargs(2, 1), 3)
    self.assertEquals(returnValueBasedOnArgsKwargs(3, 3), 6)


if __name__ == '__main__':
  unittest.main(verbosity=2)
