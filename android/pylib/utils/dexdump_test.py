#! /usr/bin/env python
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import unittest
from xml.etree import ElementTree

from pylib.utils import dexdump

# pylint: disable=protected-access


class DexdumpXMLParseTest(unittest.TestCase):

  def testParseRootXmlNode(self):
    example_xml_string = (
        '<api>'
        '<package name="com.foo.bar1">'
        '<class'
        '  name="Class1"'
        '  extends="java.lang.Object"'
        '  abstract="false"'
        '  static="false"'
        '  final="true"'
        '  visibility="public">'
        '<method'
        '  name="class1Method1"'
        '  return="java.lang.String"'
        '  abstract="false"'
        '  native="false"'
        '  synchronized="false"'
        '  static="false"'
        '  final="false"'
        '  visibility="public">'
        '</method>'
        '<method'
        '  name="class1Method2"'
        '  return="viod"'
        '  abstract="false"'
        '  native="false"'
        '  synchronized="false"'
        '  static="false"'
        '  final="false"'
        '  visibility="public">'
        '</method>'
        '</class>'
        '<class'
        '  name="Class2"'
        '  extends="java.lang.Object"'
        '  abstract="false"'
        '  static="false"'
        '  final="true"'
        '  visibility="public">'
        '<method'
        '  name="class2Method1"'
        '  return="java.lang.String"'
        '  abstract="false"'
        '  native="false"'
        '  synchronized="false"'
        '  static="false"'
        '  final="false"'
        '  visibility="public">'
        '</method>'
        '</class>'
        '</package>'
        '<package name="com.foo.bar2">'
        '</package>'
        '<package name="com.foo.bar3">'
        '</package>'
        '</api>')

    actual = dexdump._ParseRootNode(
        ElementTree.fromstring(example_xml_string))

    expected = {
      'com.foo.bar1' : {
        'classes': {
          'Class1': {
            'methods': ['class1Method1', 'class1Method2'],
            'superclass': 'java.lang.Object',
          },
          'Class2': {
            'methods': ['class2Method1'],
            'superclass': 'java.lang.Object',
          }
        },
      },
      'com.foo.bar2' : {'classes': {}},
      'com.foo.bar3' : {'classes': {}},
    }
    self.assertEquals(expected, actual)

  def testParsePackageNode(self):
    example_xml_string = (
        '<package name="com.foo.bar">'
        '<class name="Class1" extends="java.lang.Object">'
        '</class>'
        '<class name="Class2" extends="java.lang.Object">'
        '</class>'
        '</package>')


    actual = dexdump._ParsePackageNode(
        ElementTree.fromstring(example_xml_string))

    expected = {
      'classes': {
        'Class1': {
          'methods': [],
          'superclass': 'java.lang.Object',
        },
        'Class2': {
          'methods': [],
          'superclass': 'java.lang.Object',
        },
      },
    }
    self.assertEquals(expected, actual)

  def testParseClassNode(self):
    example_xml_string = (
        '<class name="Class1" extends="java.lang.Object">'
        '<method name="method1">'
        '</method>'
        '<method name="method2">'
        '</method>'
        '</class>')

    actual = dexdump._ParseClassNode(
        ElementTree.fromstring(example_xml_string))

    expected = {
      'methods': ['method1', 'method2'],
      'superclass': 'java.lang.Object',
    }
    self.assertEquals(expected, actual)


if __name__ == '__main__':
  unittest.main()
