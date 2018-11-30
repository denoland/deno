#!/usr/bin/env python
#
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
"""Tests for java_deobfuscate."""

import argparse
import os
import subprocess
import sys
import tempfile
import unittest

# Set by command-line argument.
_JAVA_DEOBFUSCATE_PATH = None

LINE_PREFIXES = [
    '',
    # logcat -v threadtime
    '09-08 14:38:35.535 18029 18084 E qcom_sensors_hal: ',
    # logcat
    'W/GCM     (15158): ',
    'W/GCM     (  158): ',
]

TEST_MAP = """\
this.was.Deobfuscated -> FOO:
    int[] FontFamily -> a
    1:3:void someMethod(int,android.os.Bundle):65:65 -> bar
"""

TEST_DATA = """\
Here is a FOO
Here is a FOO baz
Here is a "FOO" baz
Here is a "FOO.bar" baz
Here it is: FOO
Here it is: FOO.bar
SomeError: SomeFrameworkClass in isTestClass for FOO
Here is a FOO.bar
Here is a FOO.bar baz
END FOO#bar
new-instance 3810 (LSome/Framework/Class;) in LFOO;
FOO: Error message
\tat FOO.bar(PG:1)
""".splitlines(True)

EXPECTED_OUTPUT = """\
Here is a this.was.Deobfuscated
Here is a FOO baz
Here is a "this.was.Deobfuscated" baz
Here is a "this.was.Deobfuscated.someMethod" baz
Here it is: this.was.Deobfuscated
Here it is: this.was.Deobfuscated.someMethod
SomeError: SomeFrameworkClass in isTestClass for this.was.Deobfuscated
Here is a this.was.Deobfuscated.someMethod
Here is a FOO.bar baz
END this.was.Deobfuscated#someMethod
new-instance 3810 (LSome/Framework/Class;) in Lthis/was/Deobfuscated;
this.was.Deobfuscated: Error message
\tat this.was.Deobfuscated.someMethod(Deobfuscated.java:65)
""".splitlines(True)


class JavaDeobfuscateTest(unittest.TestCase):

  def __init__(self, *args, **kwargs):
    super(JavaDeobfuscateTest, self).__init__(*args, **kwargs)
    self._map_file = None

  def setUp(self):
    self._map_file = tempfile.NamedTemporaryFile()
    self._map_file.write(TEST_MAP)
    self._map_file.flush()

  def tearDown(self):
    if self._map_file:
      self._map_file.close()

  def _testImpl(self, input_lines=None, expected_output_lines=None,
                prefix=''):
    self.assertTrue(bool(input_lines) == bool(expected_output_lines))

    if not input_lines:
      input_lines = [prefix + x for x in TEST_DATA]
    if not expected_output_lines:
      expected_output_lines = [prefix + x for x in EXPECTED_OUTPUT]

    cmd = [_JAVA_DEOBFUSCATE_PATH, self._map_file.name]
    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    proc_output, _ = proc.communicate(''.join(input_lines))
    actual_output_lines = proc_output.splitlines(True)
    for actual, expected in zip(actual_output_lines, expected_output_lines):
      self.assertTrue(
          actual == expected or actual.replace('bar', 'someMethod') == expected,
          msg=''.join([
              'Deobfuscation failed.\n',
              '  actual:   %s' % actual,
              '  expected: %s' % expected]))

  def testNoPrefix(self):
    self._testImpl(prefix='')

  def testThreadtimePrefix(self):
    self._testImpl(prefix='09-08 14:38:35.535 18029 18084 E qcom_sensors_hal: ')

  def testStandardPrefix(self):
    self._testImpl(prefix='W/GCM     (15158): ')

  def testStandardPrefixWithPadding(self):
    self._testImpl(prefix='W/GCM     (  158): ')

  @unittest.skip('causes java_deobfuscate to hang, see crbug.com/876539')
  def testIndefiniteHang(self):
    # Test for crbug.com/876539.
    self._testImpl(
        input_lines=[
            'VFY: unable to resolve virtual method 2: LFOO;'
                + '.onDescendantInvalidated '
                + '(Landroid/view/View;Landroid/view/View;)V',
        ],
        expected_output_lines=[
            'VFY: unable to resolve virtual method 2: Lthis.was.Deobfuscated;'
                + '.onDescendantInvalidated '
                + '(Landroid/view/View;Landroid/view/View;)V',
        ])


if __name__ == '__main__':
  parser = argparse.ArgumentParser()
  parser.add_argument('--java-deobfuscate-path', type=os.path.realpath,
                      required=True)
  known_args, unittest_args = parser.parse_known_args()
  _JAVA_DEOBFUSCATE_PATH = known_args.java_deobfuscate_path
  unittest_args = [sys.argv[0]] + unittest_args
  unittest.main(argv=unittest_args)
