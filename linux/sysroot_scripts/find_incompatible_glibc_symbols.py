#!/usr/bin/env python
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Find incompatible symbols in glibc and output a list of replacements.
"""

import re
import sys

# This constant comes from https://crbug.com/580892
MAX_ALLOWED_GLIBC_VERSION = [2, 17]


def get_replacements(nm_file, max_allowed_glibc_version):
  symbol_format = re.compile('\S+ \S+ ([^@]+)@@?(\S+)\n')
  version_format = re.compile('GLIBC_[0-9\.]+')
  symbols = {}
  for line in nm_file:
    m = re.match(symbol_format, line)
    symbol = m.group(1)
    version = m.group(2)
    if not re.match(version_format, version):
      continue
    if symbol in symbols:
      symbols[symbol].add(version)
    else:
      symbols[symbol] = set([version])

  replacements = []
  for symbol, versions in symbols.iteritems():
    if len(versions) <= 1:
      continue
    versions_parsed = [[
        int(part) for part in version.lstrip('GLIBC_').split('.')
    ] for version in versions]
    if (max(versions_parsed) > max_allowed_glibc_version and
        min(versions_parsed) <= max_allowed_glibc_version):
      # Use the newest allowed version of the symbol.
      replacement_version_parsed = max([
          version for version in versions_parsed
          if version <= max_allowed_glibc_version
      ])
      replacement_version = 'GLIBC_' + '.'.join(
          [str(part) for part in replacement_version_parsed])
      replacements.append('__asm__(".symver %s, %s@%s");' %
                          (symbol, symbol, replacement_version))
  return sorted(replacements)


if __name__ == '__main__':
  replacements = get_replacements(sys.stdin, MAX_ALLOWED_GLIBC_VERSION)
  if replacements:
    print('// Chromium-specific hack.')
    print('// See explanation in sysroot-creator.sh.')
    for replacement in replacements:
      print replacement
