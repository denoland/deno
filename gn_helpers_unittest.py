# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import gn_helpers
import unittest

class UnitTest(unittest.TestCase):
  def test_ToGNString(self):
    self.assertEqual(
        gn_helpers.ToGNString([1, 'two', [ '"thr$\\', True, False, [] ]]),
        '[ 1, "two", [ "\\"thr\\$\\\\", true, false, [  ] ] ]')

  def test_UnescapeGNString(self):
    # Backslash followed by a \, $, or " means the folling character without
    # the special meaning. Backslash followed by everything else is a literal.
    self.assertEqual(
        gn_helpers.UnescapeGNString('\\as\\$\\\\asd\\"'),
        '\\as$\\asd"')

  def test_FromGNString(self):
    self.assertEqual(
        gn_helpers.FromGNString('[1, -20, true, false,["as\\"", []]]'),
        [ 1, -20, True, False, [ 'as"', [] ] ])

    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('123 456')
      parser.Parse()

  def test_ParseBool(self):
    parser = gn_helpers.GNValueParser('true')
    self.assertEqual(parser.Parse(), True)

    parser = gn_helpers.GNValueParser('false')
    self.assertEqual(parser.Parse(), False)

  def test_ParseNumber(self):
    parser = gn_helpers.GNValueParser('123')
    self.assertEqual(parser.ParseNumber(), 123)

    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('')
      parser.ParseNumber()
    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('a123')
      parser.ParseNumber()

  def test_ParseString(self):
    parser = gn_helpers.GNValueParser('"asdf"')
    self.assertEqual(parser.ParseString(), 'asdf')

    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('')  # Empty.
      parser.ParseString()
    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('asdf')  # Unquoted.
      parser.ParseString()
    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('"trailing')  # Unterminated.
      parser.ParseString()

  def test_ParseList(self):
    parser = gn_helpers.GNValueParser('[1,]')  # Optional end comma OK.
    self.assertEqual(parser.ParseList(), [ 1 ])

    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('')  # Empty.
      parser.ParseList()
    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('asdf')  # No [].
      parser.ParseList()
    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('[1, 2')  # Unterminated
      parser.ParseList()
    with self.assertRaises(gn_helpers.GNException):
      parser = gn_helpers.GNValueParser('[1 2]')  # No separating comma.
      parser.ParseList()

  def test_FromGNArgs(self):
    # Booleans and numbers should work; whitespace is allowed works.
    self.assertEqual(gn_helpers.FromGNArgs('foo = true\nbar = 1\n'),
                     {'foo': True, 'bar': 1})

    # Whitespace is not required; strings should also work.
    self.assertEqual(gn_helpers.FromGNArgs('foo="bar baz"'),
                     {'foo': 'bar baz'})

    # Lists should work.
    self.assertEqual(gn_helpers.FromGNArgs('foo=[1, 2, 3]'),
                     {'foo': [1, 2, 3]})

    # Empty strings should return an empty dict.
    self.assertEqual(gn_helpers.FromGNArgs(''), {})
    self.assertEqual(gn_helpers.FromGNArgs(' \n '), {})

    # Non-identifiers should raise an exception.
    with self.assertRaises(gn_helpers.GNException):
      gn_helpers.FromGNArgs('123 = true')

    # References to other variables should raise an exception.
    with self.assertRaises(gn_helpers.GNException):
      gn_helpers.FromGNArgs('foo = bar')

    # References to functions should raise an exception.
    with self.assertRaises(gn_helpers.GNException):
      gn_helpers.FromGNArgs('foo = exec_script("//build/baz.py")')

    # Underscores in identifiers should work.
    self.assertEqual(gn_helpers.FromGNArgs('_foo = true'),
                     {'_foo': True})
    self.assertEqual(gn_helpers.FromGNArgs('foo_bar = true'),
                     {'foo_bar': True})
    self.assertEqual(gn_helpers.FromGNArgs('foo_=true'),
                     {'foo_': True})

if __name__ == '__main__':
  unittest.main()
