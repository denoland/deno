#!/usr/bin/env python
#
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Writes dependency ordered list of native libraries.

The list excludes any Android system libraries, as those are not bundled with
the APK.

This list of libraries is used for several steps of building an APK.
In the component build, the --input-libraries only needs to be the top-level
library (i.e. libcontent_shell_content_view). This will then use readelf to
inspect the shared libraries and determine the full list of (non-system)
libraries that should be included in the APK.
"""

# TODO(cjhopman): See if we can expose the list of library dependencies from
# gyp, rather than calculating it ourselves.
# http://crbug.com/225558

import optparse
import os
import re
import sys

from util import build_utils

_readelf = None

_library_re = re.compile(
    '.*NEEDED.*Shared library: \[(?P<library_name>.+)\]')

_library_path_map = {}


def SetReadelfPath(path):
  global _readelf
  _readelf = path


def CallReadElf(library_or_executable):
  assert _readelf is not None
  readelf_cmd = [_readelf, '-d', library_or_executable]
  return build_utils.CheckOutput(readelf_cmd)


def GetDependencies(library_or_executable):
  elf = CallReadElf(library_or_executable)
  deps = []
  for l in _library_re.findall(elf):
    p = _library_path_map.get(l)
    if p is not None:
      deps.append(p)
  return deps


def GetSortedTransitiveDependencies(libraries):
  """Returns all transitive library dependencies in dependency order."""
  return build_utils.GetSortedTransitiveDependencies(
      libraries, GetDependencies)


def main():
  parser = optparse.OptionParser()
  build_utils.AddDepfileOption(parser)

  parser.add_option('--readelf', help='Path to the readelf binary.')
  parser.add_option('--runtime-deps',
      help='A file created for the target using write_runtime_deps.')
  parser.add_option('--exclude-shared-libraries',
      help='List of shared libraries to exclude from the output.')
  parser.add_option('--output', help='Path to the generated .json file.')

  options, _ = parser.parse_args(build_utils.ExpandFileArgs(sys.argv[1:]))

  SetReadelfPath(options.readelf)

  unsorted_lib_paths = []
  exclude_shared_libraries = []
  if options.exclude_shared_libraries:
    exclude_shared_libraries = options.exclude_shared_libraries.split(',')
  for f in open(options.runtime_deps):
    f = f[:-1]
    if f.endswith('.so'):
      p = f.replace('lib.unstripped/', '')
      if os.path.basename(p) in exclude_shared_libraries:
        continue
      unsorted_lib_paths.append(p)
      _library_path_map[os.path.basename(p)] = p

  lib_paths = GetSortedTransitiveDependencies(unsorted_lib_paths)

  libraries = [os.path.basename(l) for l in lib_paths]

  # Convert to "base" library names: e.g. libfoo.so -> foo
  java_libraries_list = (
      '{%s}' % ','.join(['"%s"' % s[3:-3] for s in libraries]))

  out_json = {
      'libraries': libraries,
      'lib_paths': lib_paths,
      'java_libraries_list': java_libraries_list
      }
  build_utils.WriteJson(
      out_json,
      options.output,
      only_if_changed=True)

  if options.depfile:
    build_utils.WriteDepfile(options.depfile, options.output, libraries,
                             add_pydeps=False)


if __name__ == '__main__':
  sys.exit(main())
