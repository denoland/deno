# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
import re
import tempfile

from devil.utils import cmd_helper
from pylib import constants


_PROGUARD_CLASS_RE = re.compile(r'\s*?- Program class:\s*([\S]+)$')
_PROGUARD_SUPERCLASS_RE = re.compile(r'\s*?  Superclass:\s*([\S]+)$')
_PROGUARD_SECTION_RE = re.compile(
    r'^(Interfaces|Constant Pool|Fields|Methods|Class file attributes) '
    r'\(count = \d+\):$')
_PROGUARD_METHOD_RE = re.compile(r'\s*?- Method:\s*(\S*)[(].*$')
_PROGUARD_ANNOTATION_RE = re.compile(r'^(\s*?)- Annotation \[L(\S*);\]:$')
_ELEMENT_PRIMITIVE = 0
_ELEMENT_ARRAY = 1
_ELEMENT_ANNOTATION = 2
_PROGUARD_ELEMENT_RES = [
  (_ELEMENT_PRIMITIVE,
   re.compile(r'^(\s*?)- Constant element value \[(\S*) .*\]$')),
  (_ELEMENT_ARRAY,
   re.compile(r'^(\s*?)- Array element value \[(\S*)\]:$')),
  (_ELEMENT_ANNOTATION,
   re.compile(r'^(\s*?)- Annotation element value \[(\S*)\]:$'))
]
_PROGUARD_INDENT_WIDTH = 2
_PROGUARD_ANNOTATION_VALUE_RE = re.compile(r'^(\s*?)- \S+? \[(.*)\]$')


def _GetProguardPath():
  # Use the one in lib.java rather than source tree because it is the one that
  # is added to swarming .isolate files.
  return os.path.join(
      constants.GetOutDirectory(), 'lib.java', 'third_party', 'proguard',
      'proguard603.jar')


def Dump(jar_path):
  """Dumps class and method information from a JAR into a dict via proguard.

  Args:
    jar_path: An absolute path to the JAR file to dump.
  Returns:
    A dict in the following format:
      {
        'classes': [
          {
            'class': '',
            'superclass': '',
            'annotations': {/* dict -- see below */},
            'methods': [
              {
                'method': '',
                'annotations': {/* dict -- see below */},
              },
              ...
            ],
          },
          ...
        ],
      }

    Annotations dict format:
      {
        'empty-annotation-class-name': None,
        'annotation-class-name': {
          'field': 'primitive-value',
          'field': [ 'array-item-1', 'array-item-2', ... ],
          'field': {
            /* Object value */
            'field': 'primitive-value',
            'field': [ 'array-item-1', 'array-item-2', ... ],
            'field': { /* Object value */ }
          }
        }
      }

    Note that for top-level annotations their class names are used for
    identification, whereas for any nested annotations the corresponding
    field names are used.

    One drawback of this approach is that an array containing empty
    annotation classes will be represented as an array of 'None' values,
    thus it will not be possible to find out annotation class names.
    On the other hand, storing both annotation class name and the field name
    would produce a very complex JSON.
  """

  with tempfile.NamedTemporaryFile() as proguard_output:
    cmd_helper.GetCmdStatusAndOutput([
        'java',
        '-jar', _GetProguardPath(),
        '-injars', jar_path,
        '-dontshrink', '-dontoptimize', '-dontobfuscate', '-dontpreverify',
        '-dump', proguard_output.name])
    return Parse(proguard_output)

class _AnnotationElement(object):
  def __init__(self, name, ftype, depth):
    self.ref = None
    self.name = name
    self.ftype = ftype
    self.depth = depth

class _ParseState(object):
  _INITIAL_VALUES = (lambda: None, list, dict)
  # Empty annotations are represented as 'None', not as an empty dictionary.
  _LAZY_INITIAL_VALUES = (lambda: None, list, lambda: None)

  def __init__(self):
    self._class_result = None
    self._method_result = None
    self._parse_annotations = False
    self._annotation_stack = []

  def ResetPerSection(self, section_name):
    self.InitMethod(None)
    self._parse_annotations = (
      section_name in ['Class file attributes', 'Methods'])

  def ParseAnnotations(self):
    return self._parse_annotations

  def CreateAndInitClass(self, class_name):
    self.InitMethod(None)
    self._class_result = {
      'class': class_name,
      'superclass': '',
      'annotations': {},
      'methods': [],
    }
    return self._class_result

  def HasCurrentClass(self):
    return bool(self._class_result)

  def SetSuperClass(self, superclass):
    assert self.HasCurrentClass()
    self._class_result['superclass'] = superclass

  def InitMethod(self, method_name):
    self._annotation_stack = []
    if method_name:
      self._method_result = {
        'method': method_name,
        'annotations': {},
      }
      self._class_result['methods'].append(self._method_result)
    else:
      self._method_result = None

  def InitAnnotation(self, annotation, depth):
    if not self._annotation_stack:
      # Add a fake parent element comprising 'annotations' dictionary,
      # so we can work uniformly with both top-level and nested annotations.
      annotations = _AnnotationElement(
        '<<<top level>>>', _ELEMENT_ANNOTATION, depth - 1)
      if self._method_result:
        annotations.ref = self._method_result['annotations']
      else:
        annotations.ref = self._class_result['annotations']
      self._annotation_stack = [annotations]
    self._BacktrackAnnotationStack(depth)
    if not self.HasCurrentAnnotation():
      self._annotation_stack.append(
        _AnnotationElement(annotation, _ELEMENT_ANNOTATION, depth))
    self._CreateAnnotationPlaceHolder(self._LAZY_INITIAL_VALUES)

  def HasCurrentAnnotation(self):
    return len(self._annotation_stack) > 1

  def InitAnnotationField(self, field, field_type, depth):
    self._BacktrackAnnotationStack(depth)
    # Create the parent representation, if needed. E.g. annotations
    # are represented with `None`, not with `{}` until they receive the first
    # field.
    self._CreateAnnotationPlaceHolder(self._INITIAL_VALUES)
    if self._annotation_stack[-1].ftype == _ELEMENT_ARRAY:
      # Nested arrays are not allowed in annotations.
      assert not field_type == _ELEMENT_ARRAY
      # Use array index instead of bogus field name.
      field = len(self._annotation_stack[-1].ref)
    self._annotation_stack.append(_AnnotationElement(field, field_type, depth))
    self._CreateAnnotationPlaceHolder(self._LAZY_INITIAL_VALUES)

  def UpdateCurrentAnnotationFieldValue(self, value, depth):
    self._BacktrackAnnotationStack(depth)
    self._InitOrUpdateCurrentField(value)

  def _CreateAnnotationPlaceHolder(self, constructors):
    assert self.HasCurrentAnnotation()
    field = self._annotation_stack[-1]
    if field.ref is None:
      field.ref = constructors[field.ftype]()
      self._InitOrUpdateCurrentField(field.ref)

  def _BacktrackAnnotationStack(self, depth):
    stack = self._annotation_stack
    while len(stack) > 0 and stack[-1].depth >= depth:
      stack.pop()

  def _InitOrUpdateCurrentField(self, value):
    assert self.HasCurrentAnnotation()
    parent = self._annotation_stack[-2]
    assert not parent.ref is None
    # There can be no nested constant element values.
    assert parent.ftype in [_ELEMENT_ARRAY, _ELEMENT_ANNOTATION]
    field = self._annotation_stack[-1]
    if isinstance(value, str) and not field.ftype == _ELEMENT_PRIMITIVE:
      # The value comes from the output parser via
      # UpdateCurrentAnnotationFieldValue, and should be a value of a constant
      # element. If it isn't, just skip it.
      return
    if parent.ftype == _ELEMENT_ARRAY and field.name >= len(parent.ref):
      parent.ref.append(value)
    else:
      parent.ref[field.name] = value


def _GetDepth(prefix):
  return len(prefix) // _PROGUARD_INDENT_WIDTH

def Parse(proguard_output):
  results = {
    'classes': [],
  }

  state = _ParseState()

  for line in proguard_output:
    line = line.strip('\r\n')

    m = _PROGUARD_CLASS_RE.match(line)
    if m:
      results['classes'].append(
        state.CreateAndInitClass(m.group(1).replace('/', '.')))
      continue

    if not state.HasCurrentClass():
      continue

    m = _PROGUARD_SUPERCLASS_RE.match(line)
    if m:
      state.SetSuperClass(m.group(1).replace('/', '.'))
      continue

    m = _PROGUARD_SECTION_RE.match(line)
    if m:
      state.ResetPerSection(m.group(1))
      continue

    m = _PROGUARD_METHOD_RE.match(line)
    if m:
      state.InitMethod(m.group(1))
      continue

    if not state.ParseAnnotations():
      continue

    m = _PROGUARD_ANNOTATION_RE.match(line)
    if m:
      # Ignore the annotation package.
      state.InitAnnotation(m.group(2).split('/')[-1], _GetDepth(m.group(1)))
      continue

    if state.HasCurrentAnnotation():
      m = None
      for (element_type, element_re) in _PROGUARD_ELEMENT_RES:
        m = element_re.match(line)
        if m:
          state.InitAnnotationField(
            m.group(2), element_type, _GetDepth(m.group(1)))
          break
      if m:
        continue
      m = _PROGUARD_ANNOTATION_VALUE_RE.match(line)
      if m:
        state.UpdateCurrentAnnotationFieldValue(
          m.group(2), _GetDepth(m.group(1)))
      else:
        state.InitMethod(None)

  return results
