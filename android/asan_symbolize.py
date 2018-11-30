#!/usr/bin/env python
#
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.


import collections
import optparse
import os
import re
import sys

from pylib import constants
from pylib.constants import host_paths

# pylint: disable=wrong-import-order
# Uses symbol.py from third_party/android_platform, not python's.
with host_paths.SysPath(
    host_paths.ANDROID_PLATFORM_DEVELOPMENT_SCRIPTS_PATH,
    position=0):
  import symbol


_RE_ASAN = re.compile(r'(.*?)(#\S*?)\s+(\S*?)\s+\((.*?)\+(.*?)\)')

# This named tuple models a parsed Asan log line.
AsanParsedLine = collections.namedtuple('AsanParsedLine',
                                        'prefix,library,pos,rel_address')

# This named tuple models an Asan log line. 'raw' is the raw content
# while 'parsed' is None or an AsanParsedLine instance.
AsanLogLine = collections.namedtuple('AsanLogLine', 'raw,parsed')

def _ParseAsanLogLine(line):
  """Parse line into corresponding AsanParsedLine value, if any, or None."""
  m = re.match(_RE_ASAN, line)
  if not m:
    return None
  return AsanParsedLine(prefix=m.group(1),
                        library=m.group(4),
                        pos=m.group(2),
                        rel_address='%08x' % int(m.group(5), 16))

def _FindASanLibraries():
  asan_lib_dir = os.path.join(host_paths.DIR_SOURCE_ROOT,
                              'third_party', 'llvm-build',
                              'Release+Asserts', 'lib')
  asan_libs = []
  for src_dir, _, files in os.walk(asan_lib_dir):
    asan_libs += [os.path.relpath(os.path.join(src_dir, f))
                  for f in files
                  if f.endswith('.so')]
  return asan_libs


def _TranslateLibPath(library, asan_libs):
  for asan_lib in asan_libs:
    if os.path.basename(library) == os.path.basename(asan_lib):
      return '/' + asan_lib
  # pylint: disable=no-member
  return symbol.TranslateLibPath(library)


def _PrintSymbolized(asan_input, arch):
  """Print symbolized logcat output for Asan symbols.

  Args:
    asan_input: list of input lines.
    arch: Target CPU architecture.
  """
  asan_libs = _FindASanLibraries()

  # Maps library -> [ AsanParsedLine... ]
  libraries = collections.defaultdict(list)

  asan_log_lines = []
  for line in asan_input:
    line = line.rstrip()
    parsed = _ParseAsanLogLine(line)
    if parsed:
      libraries[parsed.library].append(parsed)
    asan_log_lines.append(AsanLogLine(raw=line, parsed=parsed))

  # Maps library -> { address -> [(symbol, location, obj_sym_with_offset)...] }
  all_symbols = collections.defaultdict(dict)

  for library, items in libraries.iteritems():
    libname = _TranslateLibPath(library, asan_libs)
    lib_relative_addrs = set([i.rel_address for i in items])
    # pylint: disable=no-member
    info_dict = symbol.SymbolInformationForSet(libname,
                                               lib_relative_addrs,
                                               True,
                                               cpu_arch=arch)
    if info_dict:
      all_symbols[library] = info_dict

  for log_line in asan_log_lines:
    m = log_line.parsed
    if (m and m.library in all_symbols and
        m.rel_address in all_symbols[m.library]):
      # NOTE: all_symbols[lib][address] is a never-emtpy list of tuples.
      # NOTE: The documentation for SymbolInformationForSet() indicates
      # that usually one wants to display the last list item, not the first.
      # The code below takes the first, is this the best choice here?
      s = all_symbols[m.library][m.rel_address][0]
      print '%s%s %s %s' % (m.prefix, m.pos, s[0], s[1])
    else:
      print log_line.raw


def main():
  parser = optparse.OptionParser()
  parser.add_option('-l', '--logcat',
                    help='File containing adb logcat output with ASan stacks. '
                         'Use stdin if not specified.')
  parser.add_option('--output-directory',
                    help='Path to the root build directory.')
  parser.add_option('--arch', default='arm',
                    help='CPU architecture name')
  options, _ = parser.parse_args()

  if options.output_directory:
    constants.SetOutputDirectory(options.output_directory)
  # Do an up-front test that the output directory is known.
  constants.CheckOutputDirectory()

  if options.logcat:
    asan_input = file(options.logcat, 'r')
  else:
    asan_input = sys.stdin

  _PrintSymbolized(asan_input.readlines(), options.arch)


if __name__ == "__main__":
  sys.exit(main())
