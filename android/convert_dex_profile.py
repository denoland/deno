#!/usr/bin/env vpython
#
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import collections
import logging
import re
import subprocess
import sys

DEX_CLASS_NAME_RE = re.compile(r'\'L(?P<class_name>[^;]+);\'')
DEX_METHOD_NAME_RE = re.compile(r'\'(?P<method_name>[^\']+)\'')
DEX_METHOD_TYPE_RE = re.compile( # type descriptor method signature re
    r'\''
    r'\('
    r'(?P<method_params>[^)]*)'
    r'\)'
    r'(?P<method_return_type>[^\']+)'
    r'\'')
DEX_METHOD_LINE_NR_RE = re.compile(r'line=(?P<line_number>\d+)')

PROFILE_METHOD_RE = re.compile(
    r'(?P<tags>[HSP]+)' # tags such as H/S/P
    r'(?P<class_name>L[^;]+;)' # class name in type descriptor format
    r'->(?P<method_name>[^(]+)'
    r'\((?P<method_params>[^)]*)\)'
    r'(?P<method_return_type>.+)')

PROGUARD_CLASS_MAPPING_RE = re.compile(
    r'(?P<original_name>[^ ]+)'
    r' -> '
    r'(?P<obfuscated_name>[^:]+):')
PROGUARD_METHOD_MAPPING_RE = re.compile(
    # line_start:line_end: (optional)
    r'((?P<line_start>\d+):(?P<line_end>\d+):)?'
    r'(?P<return_type>[^ ]+)' # original method return type
    # original method class name (if exists)
    r' (?:(?P<original_method_class>[a-zA-Z_\d.$]+)\.)?'
    r'(?P<original_method_name>[^.\(]+)'
    r'\((?P<params>[^\)]*)\)' # original method params
    r'(?:[^ ]*)' # original method line numbers (ignored)
    r' -> '
    r'(?P<obfuscated_name>.+)') # obfuscated method name

TYPE_DESCRIPTOR_RE = re.compile(
    r'(?P<brackets>\[*)'
    r'(?:'
    r'(?P<class_name>L[^;]+;)'
    r'|'
    r'[VZBSCIJFD]'
    r')')

DOT_NOTATION_MAP = {
    '': '',
    'boolean': 'Z',
    'byte': 'B',
    'void': 'V',
    'short': 'S',
    'char': 'C',
    'int': 'I',
    'long': 'J',
    'float': 'F',
    'double': 'D'
}

class Method(object):
  def __init__(self, name, class_name, param_types=None, return_type=None):
    self.name = name
    self.class_name = class_name
    self.param_types = param_types
    self.return_type = return_type

  def __str__(self):
    return '{}->{}({}){}'.format(self.class_name, self.name,
        self.param_types or '', self.return_type or '')

  def __repr__(self):
    return 'Method<{}->{}({}){}>'.format(self.class_name, self.name,
        self.param_types or '', self.return_type or '')

  def __cmp__(self, other):
    return cmp((self.class_name, self.name, self.param_types, self.return_type),
        (other.class_name, other.name, other.param_types, other.return_type))

  def __hash__(self):
    # only hash name and class_name since other fields may not be set yet.
    return hash((self.name, self.class_name))


class Class(object):
  def __init__(self, name):
    self.name = name
    self._methods = []

  def AddMethod(self, method, line_numbers):
    self._methods.append((method, set(line_numbers)))

  def FindMethodsAtLine(self, method_name, line_start, line_end=None):
    """Searches through dex class for a method given a name and line numbers

    The dex maps methods to line numbers, this method, given the a method name
    in this class as well as a start line and an optional end line (which act as
    hints as to which function in the class is being looked for), returns a list
    of possible matches (or none if none are found).

    Args:
      method_name: name of method being searched for
      line_start: start of hint range for lines in this method
      line_end: end of hint range for lines in this method (optional)

    Returns:
      A list of Method objects that could match the hints given, or None if no
      method is found.
    """
    found_methods = []
    if line_end is None:
      hint_lines = set([line_start])
    else:
      hint_lines = set(range(line_start, line_end+1))

    named_methods = [(method, l) for method, l in self._methods
                     if method.name == method_name]

    if len(named_methods) == 1:
      return [method for method, l in named_methods]
    if len(named_methods) == 0:
      return None

    for method, line_numbers in named_methods:
      if not hint_lines.isdisjoint(line_numbers):
        found_methods.append(method)

    if len(found_methods) > 0:
      if len(found_methods) > 1:
        logging.warning('ambigous methods in dex %s at lines %s in class "%s"',
            found_methods, hint_lines, self.name)
      return found_methods

    for method, line_numbers in named_methods:
      if (max(hint_lines) >= min(line_numbers)
          and min(hint_lines) <= max(line_numbers)):
        found_methods.append(method)

    if len(found_methods) > 0:
      if len(found_methods) > 1:
        logging.warning('ambigous methods in dex %s at lines %s in class "%s"',
            found_methods, hint_lines, self.name)
      return found_methods
    else:
      logging.warning('No method named "%s" in class "%s" is '
                      'mapped to lines %s', method_name, self.name, hint_lines)
      return None


class Profile(object):
  def __init__(self):
    # {Method: set(char)}
    self._methods = collections.defaultdict(set)
    self._classes = []

  def AddMethod(self, method, tags):
    for tag in tags:
      self._methods[method].add(tag)

  def AddClass(self, cls):
    self._classes.append(cls)

  def WriteToFile(self, path):
    with open(path, 'w') as output_profile:
      for cls in sorted(self._classes):
        output_profile.write(cls + '\n')
      for method in sorted(self._methods):
        tags = sorted(self._methods[method])
        line = '{}{}\n'.format(''.join(tags), str(method))
        output_profile.write(line)


class ProguardMapping(object):
  def __init__(self):
    # {Method: set(Method)}
    self._method_mapping = collections.defaultdict(set)
    # {String: String} String is class name in type descriptor format
    self._class_mapping = dict()

  def AddMethodMapping(self, from_method, to_method):
    self._method_mapping[from_method].add(to_method)

  def AddClassMapping(self, from_class, to_class):
    self._class_mapping[from_class] = to_class

  def GetMethodMapping(self, from_method):
    return self._method_mapping.get(from_method)

  def GetClassMapping(self, from_class):
    return self._class_mapping.get(from_class, from_class)

  def MapTypeDescriptor(self, type_descriptor):
    match = TYPE_DESCRIPTOR_RE.search(type_descriptor)
    assert match is not None
    class_name = match.group('class_name')
    if class_name is not None:
      return match.group('brackets') + self.GetClassMapping(class_name)
    # just a native type, return as is
    return match.group()

  def MapTypeDescriptorList(self, type_descriptor_list):
    return TYPE_DESCRIPTOR_RE.sub(
        lambda match: self.MapTypeDescriptor(match.group()),
        type_descriptor_list)


class MalformedLineException(Exception):
  def __init__(self, message, line_number):
    super(MalformedLineException, self).__init__(message)
    self.line_number = line_number

  def __str__(self):
    return self.message + ' at line {}'.format(self.line_number)


class MalformedProguardMappingException(MalformedLineException):
  pass


class MalformedProfileException(MalformedLineException):
  pass


def _RunDexDump(dexdump_path, dex_file_path):
  return subprocess.check_output([dexdump_path, dex_file_path]).splitlines()


def _ReadFile(file_path):
  with open(file_path, 'r') as f:
    return f.readlines()


def _ToTypeDescriptor(dot_notation):
  """Parses a dot notation type and returns it in type descriptor format

  eg:
  org.chromium.browser.ChromeActivity -> Lorg/chromium/browser/ChromeActivity;
  boolean -> Z
  int[] -> [I

  Args:
    dot_notation: trimmed string with a single type in dot notation format

  Returns:
    A string with the type in type descriptor format
  """
  dot_notation = dot_notation.strip()
  prefix = ''
  while dot_notation.endswith('[]'):
    prefix += '['
    dot_notation = dot_notation[:-2]
  if dot_notation in DOT_NOTATION_MAP:
    return prefix + DOT_NOTATION_MAP[dot_notation]
  return prefix + 'L' + dot_notation.replace('.', '/') + ';'


def _DotNotationListToTypeDescriptorList(dot_notation_list_string):
  """Parses a param list of dot notation format and returns it in type
  descriptor format

  eg:
  org.chromium.browser.ChromeActivity,boolean,int[] ->
      Lorg/chromium/browser/ChromeActivity;Z[I

  Args:
    dot_notation_list_string: single string with multiple comma separated types
                              in dot notation format

  Returns:
    A string with the param list in type descriptor format
  """
  return ''.join(_ToTypeDescriptor(param) for param in
      dot_notation_list_string.split(','))


def ProcessDex(dex_dump):
  """Parses dexdump output returning a dict of class names to Class objects

  Parses output of the dexdump command on a dex file and extracts information
  about classes and their respective methods and which line numbers a method is
  mapped to.

  Methods that are not mapped to any line number are ignored and not listed
  inside their respective Class objects.

  Args:
    dex_dump: An array of lines of dexdump output

  Returns:
    A dict that maps from class names in type descriptor format (but without the
    surrounding 'L' and ';') to Class objects.
  """
  # class_name: Class
  classes_by_name = {}
  current_class = None
  current_method = None
  reading_positions = False
  reading_methods = False
  method_line_numbers = []
  for line in dex_dump:
    line = line.strip()
    if line.startswith('Class descriptor'):
      # New class started, no longer reading methods.
      reading_methods = False
      current_class = Class(DEX_CLASS_NAME_RE.search(line).group('class_name'))
      classes_by_name[current_class.name] = current_class
    elif (line.startswith('Direct methods')
          or line.startswith('Virtual methods')):
      reading_methods = True
    elif reading_methods and line.startswith('name'):
      assert current_class is not None
      current_method = Method(
          DEX_METHOD_NAME_RE.search(line).group('method_name'),
          "L" + current_class.name + ";")
    elif reading_methods and line.startswith('type'):
      assert current_method is not None
      match = DEX_METHOD_TYPE_RE.search(line)
      current_method.param_types = match.group('method_params')
      current_method.return_type = match.group('method_return_type')
    elif line.startswith('positions'):
      assert reading_methods
      reading_positions = True
      method_line_numbers = []
    elif reading_positions and line.startswith('0x'):
      line_number = DEX_METHOD_LINE_NR_RE.search(line).group('line_number')
      method_line_numbers.append(int(line_number))
    elif reading_positions and line.startswith('locals'):
      if len(method_line_numbers) > 0:
        current_class.AddMethod(current_method, method_line_numbers)
      # finished reading method line numbers
      reading_positions = False
  return classes_by_name


def ProcessProguardMapping(proguard_mapping_lines, dex):
  """Parses a proguard mapping file

  This takes proguard mapping file lines and then uses the obfuscated dex to
  create a mapping of unobfuscated methods to obfuscated ones and vice versa.

  The dex is used because the proguard mapping file only has the name of the
  obfuscated methods but not their signature, thus the dex is read to look up
  which method with a specific name was mapped to the lines mentioned in the
  proguard mapping file.

  Args:
    proguard_mapping_lines: Array of strings, each is a line from the proguard
                            mapping file (in order).
    dex: a dict of class name (in type descriptor format but without the
         enclosing 'L' and ';') to a Class object.
  Returns:
    Two dicts the first maps from obfuscated methods to a set of non-obfuscated
    ones. It also maps the obfuscated class names to original class names, both
    in type descriptor format (with the enclosing 'L' and ';')
  """
  mapping = ProguardMapping()
  reverse_mapping = ProguardMapping()
  to_be_obfuscated = []
  current_class_orig = None
  current_class_obfs = None
  for index, line in enumerate(proguard_mapping_lines):
    if line.strip() == '':
      continue
    if not line.startswith(' '):
      match = PROGUARD_CLASS_MAPPING_RE.search(line)
      if match is None:
        raise MalformedProguardMappingException(
            'Malformed class mapping', index)
      current_class_orig = match.group('original_name')
      current_class_obfs = match.group('obfuscated_name')
      mapping.AddClassMapping(_ToTypeDescriptor(current_class_obfs),
                              _ToTypeDescriptor(current_class_orig))
      reverse_mapping.AddClassMapping(_ToTypeDescriptor(current_class_orig),
                                      _ToTypeDescriptor(current_class_obfs))
      continue

    assert current_class_orig is not None
    assert current_class_obfs is not None
    line = line.strip()
    match = PROGUARD_METHOD_MAPPING_RE.search(line)
    # check if is a method mapping (we ignore field mappings)
    if match is not None:
      # check if this line is an inlining by reading ahead 1 line.
      if index + 1 < len(proguard_mapping_lines):
        next_match = PROGUARD_METHOD_MAPPING_RE.search(
            proguard_mapping_lines[index+1].strip())
        if (next_match and match.group('line_start') is not None
            and next_match.group('line_start') == match.group('line_start')
            and next_match.group('line_end') == match.group('line_end')):
          continue # This is an inlining, skip

      original_method = Method(
          match.group('original_method_name'),
          _ToTypeDescriptor(
              match.group('original_method_class') or current_class_orig),
          _DotNotationListToTypeDescriptorList(match.group('params')),
          _ToTypeDescriptor(match.group('return_type')))

      if match.group('line_start') is not None:
        obfs_methods = (dex[current_class_obfs.replace('.', '/')]
            .FindMethodsAtLine(
                match.group('obfuscated_name'),
                int(match.group('line_start')),
                int(match.group('line_end'))))

        if obfs_methods is None:
          continue

        for obfs_method in obfs_methods:
          mapping.AddMethodMapping(obfs_method, original_method)
          reverse_mapping.AddMethodMapping(original_method, obfs_method)
      else:
        to_be_obfuscated.append(
            (original_method, match.group('obfuscated_name')))

  for original_method, obfuscated_name in to_be_obfuscated:
    obfuscated_method = Method(
        obfuscated_name,
        reverse_mapping.GetClassMapping(original_method.class_name),
        reverse_mapping.MapTypeDescriptorList(original_method.param_types),
        reverse_mapping.MapTypeDescriptor(original_method.return_type))
    mapping.AddMethodMapping(obfuscated_method, original_method)
    reverse_mapping.AddMethodMapping(original_method, obfuscated_method)
  return mapping, reverse_mapping


def ProcessProfile(input_profile, proguard_mapping):
  """Parses an android profile and uses the proguard mapping to (de)obfuscate it

  This takes the android profile lines and for each method or class for the
  profile, it uses the mapping to either obfuscate or deobfuscate (based on the
  provided mapping) and returns a Profile object that stores this information.

  Args:
    input_profile: array of lines of the input profile
    proguard_mapping: a proguard mapping that would map from the classes and
                      methods in the input profile to the classes and methods
                      that should be in the output profile.

  Returns:
    A Profile object that stores the information (ie list of mapped classes and
    methods + tags)
  """
  profile = Profile()
  for index, line in enumerate(input_profile):
    line = line.strip()
    if line.startswith('L'):
      profile.AddClass(proguard_mapping.GetClassMapping(line))
      continue
    match = PROFILE_METHOD_RE.search(line)
    if not match:
      raise MalformedProfileException("Malformed line", index)

    method = Method(
        match.group('method_name'),
        match.group('class_name'),
        match.group('method_params'),
        match.group('method_return_type'))

    mapped_methods = proguard_mapping.GetMethodMapping(method)
    if mapped_methods is None:
      logging.warning('No method matching "%s" has been found in the proguard '
                      'mapping file', method)
      continue

    for original_method in mapped_methods:
      profile.AddMethod(original_method, match.group('tags'))

  return profile


def main(args):
  parser = argparse.ArgumentParser()
  parser.add_argument(
      '--dexdump-path',
      required=True,
      help='Path to dexdump binary.')
  parser.add_argument(
      '--dex-path',
      required=True,
      help='Path to dex file corresponding to the proguard mapping file.')
  parser.add_argument(
      '--proguard-mapping-path',
      required=True,
      help='Path to input proguard mapping file corresponding to the dex file.')
  parser.add_argument(
      '--output-profile-path',
      required=True,
      help='Path to output profile.')
  parser.add_argument(
      '--input-profile-path',
      required=True,
      help='Path to output profile.')
  parser.add_argument(
      '--verbose',
      action='store_true',
      default=False,
      help='Print verbose output.')
  obfuscation = parser.add_mutually_exclusive_group(required=True)
  obfuscation.add_argument('--obfuscate', action='store_true',
      help='Indicates to output an obfuscated profile given a deobfuscated '
     'one.')
  obfuscation.add_argument('--deobfuscate', dest='obfuscate',
      action='store_false', help='Indicates to output a deobfuscated profile '
      'given an obfuscated one.')
  options = parser.parse_args(args)

  if options.verbose:
    log_level = logging.WARNING
  else:
    log_level = logging.ERROR
  logging.basicConfig(format='%(levelname)s: %(message)s', level=log_level)

  dex = ProcessDex(_RunDexDump(options.dexdump_path, options.dex_path))
  proguard_mapping, reverse_proguard_mapping = ProcessProguardMapping(
      _ReadFile(options.proguard_mapping_path), dex)
  if options.obfuscate:
    profile = ProcessProfile(
        _ReadFile(options.input_profile_path),
        reverse_proguard_mapping)
  else:
    profile = ProcessProfile(
        _ReadFile(options.input_profile_path),
        proguard_mapping)
  profile.WriteToFile(options.output_profile_path)


if __name__ == '__main__':
  main(sys.argv[1:])
