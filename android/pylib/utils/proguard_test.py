#! /usr/bin/env python
# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import unittest

from pylib.utils import proguard

class TestParse(unittest.TestCase):

  def setUp(self):
    self.maxDiff = None

  def testClass(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       '  Superclass: java/lang/Object'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': 'java.lang.Object',
          'annotations': {},
          'methods': []
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testMethod(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Methods (count = 1):',
       '- Method:       <init>()V'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {},
          'methods': [
            {
              'method': '<init>',
              'annotations': {}
            }
          ]
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testClassAnnotation(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Class file attributes (count = 3):',
       '  - Annotation [Lorg/example/Annotation;]:',
       '  - Annotation [Lorg/example/AnnotationWithValue;]:',
       '    - Constant element value [attr \'13\']',
       '      - Utf8 [val]',
       '  - Annotation [Lorg/example/AnnotationWithTwoValues;]:',
       '    - Constant element value [attr1 \'13\']',
       '      - Utf8 [val1]',
       '    - Constant element value [attr2 \'13\']',
       '      - Utf8 [val2]'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {
            'Annotation': None,
            'AnnotationWithValue': {'attr': 'val'},
            'AnnotationWithTwoValues': {'attr1': 'val1', 'attr2': 'val2'}
          },
          'methods': []
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testClassAnnotationWithArrays(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Class file attributes (count = 3):',
       '  - Annotation [Lorg/example/AnnotationWithEmptyArray;]:',
       '    - Array element value [arrayAttr]:',
       '  - Annotation [Lorg/example/AnnotationWithOneElemArray;]:',
       '    - Array element value [arrayAttr]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val]',
       '  - Annotation [Lorg/example/AnnotationWithTwoElemArray;]:',
       '    - Array element value [arrayAttr]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val1]',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val2]'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {
            'AnnotationWithEmptyArray': {'arrayAttr': []},
            'AnnotationWithOneElemArray': {'arrayAttr': ['val']},
            'AnnotationWithTwoElemArray': {'arrayAttr': ['val1', 'val2']}
          },
          'methods': []
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testNestedClassAnnotations(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Class file attributes (count = 1):',
       '  - Annotation [Lorg/example/OuterAnnotation;]:',
       '    - Constant element value [outerAttr \'13\']',
       '      - Utf8 [outerVal]',
       '    - Array element value [outerArr]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [outerArrVal1]',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [outerArrVal2]',
       '    - Annotation element value [emptyAnn]:',
       '      - Annotation [Lorg/example/EmptyAnnotation;]:',
       '    - Annotation element value [ann]:',
       '      - Annotation [Lorg/example/InnerAnnotation;]:',
       '        - Constant element value [innerAttr \'13\']',
       '          - Utf8 [innerVal]',
       '        - Array element value [innerArr]:',
       '          - Constant element value [(default) \'13\']',
       '            - Utf8 [innerArrVal1]',
       '          - Constant element value [(default) \'13\']',
       '            - Utf8 [innerArrVal2]',
       '        - Annotation element value [emptyInnerAnn]:',
       '          - Annotation [Lorg/example/EmptyAnnotation;]:'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {
            'OuterAnnotation': {
              'outerAttr': 'outerVal',
              'outerArr': ['outerArrVal1', 'outerArrVal2'],
              'emptyAnn': None,
              'ann': {
                'innerAttr': 'innerVal',
                'innerArr': ['innerArrVal1', 'innerArrVal2'],
                'emptyInnerAnn': None
              }
            }
          },
          'methods': []
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testClassArraysOfAnnotations(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Class file attributes (count = 1):',
       '   - Annotation [Lorg/example/OuterAnnotation;]:',
       '     - Array element value [arrayWithEmptyAnnotations]:',
       '       - Annotation element value [(default)]:',
       '         - Annotation [Lorg/example/EmptyAnnotation;]:',
       '       - Annotation element value [(default)]:',
       '         - Annotation [Lorg/example/EmptyAnnotation;]:',
       '     - Array element value [outerArray]:',
       '       - Annotation element value [(default)]:',
       '         - Annotation [Lorg/example/InnerAnnotation;]:',
       '           - Constant element value [innerAttr \'115\']',
       '             - Utf8 [innerVal]',
       '           - Array element value [arguments]:',
       '             - Annotation element value [(default)]:',
       '               - Annotation [Lorg/example/InnerAnnotation$Argument;]:',
       '                 - Constant element value [arg1Attr \'115\']',
       '                   - Utf8 [arg1Val]',
       '                 - Array element value [arg1Array]:',
       '                   - Constant element value [(default) \'73\']',
       '                     - Integer [11]',
       '                   - Constant element value [(default) \'73\']',
       '                     - Integer [12]',
       '             - Annotation element value [(default)]:',
       '               - Annotation [Lorg/example/InnerAnnotation$Argument;]:',
       '                 - Constant element value [arg2Attr \'115\']',
       '                   - Utf8 [arg2Val]',
       '                 - Array element value [arg2Array]:',
       '                   - Constant element value [(default) \'73\']',
       '                     - Integer [21]',
       '                   - Constant element value [(default) \'73\']',
       '                     - Integer [22]'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {
            'OuterAnnotation': {
              'arrayWithEmptyAnnotations': [None, None],
              'outerArray': [
                {
                  'innerAttr': 'innerVal',
                  'arguments': [
                    {'arg1Attr': 'arg1Val', 'arg1Array': ['11', '12']},
                    {'arg2Attr': 'arg2Val', 'arg2Array': ['21', '22']}
                  ]
                }
              ]
            }
          },
          'methods': []
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testReadFullClassFileAttributes(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Class file attributes (count = 3):',
       '  - Source file attribute:',
       '    - Utf8 [Class.java]',
       '  - Runtime visible annotations attribute:',
       '    - Annotation [Lorg/example/IntValueAnnotation;]:',
       '      - Constant element value [value \'73\']',
       '        - Integer [19]',
       '  - Inner classes attribute (count = 1)',
       '    - InnerClassesInfo:',
       '      Access flags:  0x9 = public static',
       '      - Class [org/example/Class1]',
       '      - Class [org/example/Class2]',
       '      - Utf8 [OnPageFinishedHelper]'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {
            'IntValueAnnotation': {
              'value': '19',
            }
          },
          'methods': []
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testMethodAnnotation(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Methods (count = 1):',
       '- Method:       Test()V',
       '  - Annotation [Lorg/example/Annotation;]:',
       '  - Annotation [Lorg/example/AnnotationWithValue;]:',
       '    - Constant element value [attr \'13\']',
       '      - Utf8 [val]',
       '  - Annotation [Lorg/example/AnnotationWithTwoValues;]:',
       '    - Constant element value [attr1 \'13\']',
       '      - Utf8 [val1]',
       '    - Constant element value [attr2 \'13\']',
       '      - Utf8 [val2]'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {},
          'methods': [
            {
              'method': 'Test',
              'annotations': {
                'Annotation': None,
                'AnnotationWithValue': {'attr': 'val'},
                'AnnotationWithTwoValues': {'attr1': 'val1', 'attr2': 'val2'}
              },
            }
          ]
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testMethodAnnotationWithArrays(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Methods (count = 1):',
       '- Method:       Test()V',
       '  - Annotation [Lorg/example/AnnotationWithEmptyArray;]:',
       '    - Array element value [arrayAttr]:',
       '  - Annotation [Lorg/example/AnnotationWithOneElemArray;]:',
       '    - Array element value [arrayAttr]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val]',
       '  - Annotation [Lorg/example/AnnotationWithTwoElemArray;]:',
       '    - Array element value [arrayAttr]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val1]',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val2]'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {},
          'methods': [
            {
              'method': 'Test',
              'annotations': {
                'AnnotationWithEmptyArray': {'arrayAttr': []},
                'AnnotationWithOneElemArray': {'arrayAttr': ['val']},
                'AnnotationWithTwoElemArray': {'arrayAttr': ['val1', 'val2']}
              },
            }
          ]
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testMethodAnnotationWithPrimitivesAndArrays(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Methods (count = 1):',
       '- Method:       Test()V',
       '  - Annotation [Lorg/example/AnnotationPrimitiveThenArray;]:',
       '    - Constant element value [attr \'13\']',
       '      - Utf8 [val]',
       '    - Array element value [arrayAttr]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val]',
       '  - Annotation [Lorg/example/AnnotationArrayThenPrimitive;]:',
       '    - Array element value [arrayAttr]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val]',
       '    - Constant element value [attr \'13\']',
       '      - Utf8 [val]',
       '  - Annotation [Lorg/example/AnnotationTwoArrays;]:',
       '    - Array element value [arrayAttr1]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val1]',
       '    - Array element value [arrayAttr2]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [val2]'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {},
          'methods': [
            {
              'method': 'Test',
              'annotations': {
                'AnnotationPrimitiveThenArray': {'attr': 'val',
                                                 'arrayAttr': ['val']},
                'AnnotationArrayThenPrimitive': {'arrayAttr': ['val'],
                                                 'attr': 'val'},
                'AnnotationTwoArrays': {'arrayAttr1': ['val1'],
                                        'arrayAttr2': ['val2']}
              },
            }
          ]
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testNestedMethodAnnotations(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Methods (count = 1):',
       '- Method:       Test()V',
       '  - Annotation [Lorg/example/OuterAnnotation;]:',
       '    - Constant element value [outerAttr \'13\']',
       '      - Utf8 [outerVal]',
       '    - Array element value [outerArr]:',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [outerArrVal1]',
       '      - Constant element value [(default) \'13\']',
       '        - Utf8 [outerArrVal2]',
       '    - Annotation element value [emptyAnn]:',
       '      - Annotation [Lorg/example/EmptyAnnotation;]:',
       '    - Annotation element value [ann]:',
       '      - Annotation [Lorg/example/InnerAnnotation;]:',
       '        - Constant element value [innerAttr \'13\']',
       '          - Utf8 [innerVal]',
       '        - Array element value [innerArr]:',
       '          - Constant element value [(default) \'13\']',
       '            - Utf8 [innerArrVal1]',
       '          - Constant element value [(default) \'13\']',
       '            - Utf8 [innerArrVal2]',
       '        - Annotation element value [emptyInnerAnn]:',
       '          - Annotation [Lorg/example/EmptyAnnotation;]:'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {},
          'methods': [
            {
              'method': 'Test',
              'annotations': {
                'OuterAnnotation': {
                  'outerAttr': 'outerVal',
                  'outerArr': ['outerArrVal1', 'outerArrVal2'],
                  'emptyAnn': None,
                  'ann': {
                    'innerAttr': 'innerVal',
                    'innerArr': ['innerArrVal1', 'innerArrVal2'],
                    'emptyInnerAnn': None
                  }
                }
              },
            }
          ]
        }
      ]
    }
    self.assertEquals(expected, actual)

  def testMethodArraysOfAnnotations(self):
    actual = proguard.Parse(
      ['- Program class: org/example/Test',
       'Methods (count = 1):',
       '- Method:       Test()V',
       '   - Annotation [Lorg/example/OuterAnnotation;]:',
       '     - Array element value [arrayWithEmptyAnnotations]:',
       '       - Annotation element value [(default)]:',
       '         - Annotation [Lorg/example/EmptyAnnotation;]:',
       '       - Annotation element value [(default)]:',
       '         - Annotation [Lorg/example/EmptyAnnotation;]:',
       '     - Array element value [outerArray]:',
       '       - Annotation element value [(default)]:',
       '         - Annotation [Lorg/example/InnerAnnotation;]:',
       '           - Constant element value [innerAttr \'115\']',
       '             - Utf8 [innerVal]',
       '           - Array element value [arguments]:',
       '             - Annotation element value [(default)]:',
       '               - Annotation [Lorg/example/InnerAnnotation$Argument;]:',
       '                 - Constant element value [arg1Attr \'115\']',
       '                   - Utf8 [arg1Val]',
       '                 - Array element value [arg1Array]:',
       '                   - Constant element value [(default) \'73\']',
       '                     - Integer [11]',
       '                   - Constant element value [(default) \'73\']',
       '                     - Integer [12]',
       '             - Annotation element value [(default)]:',
       '               - Annotation [Lorg/example/InnerAnnotation$Argument;]:',
       '                 - Constant element value [arg2Attr \'115\']',
       '                   - Utf8 [arg2Val]',
       '                 - Array element value [arg2Array]:',
       '                   - Constant element value [(default) \'73\']',
       '                     - Integer [21]',
       '                   - Constant element value [(default) \'73\']',
       '                     - Integer [22]'])
    expected = {
      'classes': [
        {
          'class': 'org.example.Test',
          'superclass': '',
          'annotations': {},
          'methods': [
            {
              'method': 'Test',
              'annotations': {
                'OuterAnnotation': {
                  'arrayWithEmptyAnnotations': [None, None],
                  'outerArray': [
                    {
                      'innerAttr': 'innerVal',
                      'arguments': [
                        {'arg1Attr': 'arg1Val', 'arg1Array': ['11', '12']},
                        {'arg2Attr': 'arg2Val', 'arg2Array': ['21', '22']}
                      ]
                    }
                  ]
                }
              }
            }
          ]
        }
      ]
    }
    self.assertEquals(expected, actual)


if __name__ == '__main__':
  unittest.main()
