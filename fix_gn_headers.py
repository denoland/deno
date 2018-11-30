#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Fix header files missing in GN.

This script takes the missing header files from check_gn_headers.py, and
try to fix them by adding them to the GN files.
Manual cleaning up is likely required afterwards.
"""

import argparse
import os
import re
import subprocess
import sys


def GitGrep(pattern):
  p = subprocess.Popen(
      ['git', 'grep', '-En', pattern, '--', '*.gn', '*.gni'],
      stdout=subprocess.PIPE)
  out, _ = p.communicate()
  return out, p.returncode


def ValidMatches(basename, cc, grep_lines):
  """Filter out 'git grep' matches with header files already."""
  matches = []
  for line in grep_lines:
    gnfile, linenr, contents = line.split(':')
    linenr = int(linenr)
    new = re.sub(cc, basename, contents)
    lines = open(gnfile).read().splitlines()
    assert contents in lines[linenr - 1]
    # Skip if it's already there. It could be before or after the match.
    if lines[linenr] == new:
      continue
    if lines[linenr - 2] == new:
      continue
    print '    ', gnfile, linenr, new
    matches.append((gnfile, linenr, new))
  return matches


def AddHeadersNextToCC(headers, skip_ambiguous=True):
  """Add header files next to the corresponding .cc files in GN files.

  When skip_ambiguous is True, skip if multiple .cc files are found.
  Returns unhandled headers.

  Manual cleaning up is likely required, especially if not skip_ambiguous.
  """
  edits = {}
  unhandled = []
  for filename in headers:
    filename = filename.strip()
    if not (filename.endswith('.h') or filename.endswith('.hh')):
      continue
    basename = os.path.basename(filename)
    print filename
    cc = r'\b' + os.path.splitext(basename)[0] + r'\.(cc|cpp|mm)\b'
    out, returncode = GitGrep('(/|")' + cc + '"')
    if returncode != 0 or not out:
      unhandled.append(filename)
      continue

    matches = ValidMatches(basename, cc, out.splitlines())

    if len(matches) == 0:
      continue
    if len(matches) > 1:
      print '\n[WARNING] Ambiguous matching for', filename
      for i in enumerate(matches, 1):
        print '%d: %s' % (i[0], i[1])
      print
      if skip_ambiguous:
        continue

      picked = raw_input('Pick the matches ("2,3" for multiple): ')
      try:
        matches = [matches[int(i) - 1] for i in picked.split(',')]
      except (ValueError, IndexError):
        continue

    for match in matches:
      gnfile, linenr, new = match
      print '  ', gnfile, linenr, new
      edits.setdefault(gnfile, {})[linenr] = new

  for gnfile in edits:
    lines = open(gnfile).read().splitlines()
    for l in sorted(edits[gnfile].keys(), reverse=True):
      lines.insert(l, edits[gnfile][l])
    open(gnfile, 'w').write('\n'.join(lines) + '\n')

  return unhandled


def AddHeadersToSources(headers, skip_ambiguous=True):
  """Add header files to the sources list in the first GN file.

  The target GN file is the first one up the parent directories.
  This usually does the wrong thing for _test files if the test and the main
  target are in the same .gn file.
  When skip_ambiguous is True, skip if multiple sources arrays are found.

  "git cl format" afterwards is required. Manually cleaning up duplicated items
  is likely required.
  """
  for filename in headers:
    filename = filename.strip()
    print filename
    dirname = os.path.dirname(filename)
    while not os.path.exists(os.path.join(dirname, 'BUILD.gn')):
      dirname = os.path.dirname(dirname)
    rel = filename[len(dirname) + 1:]
    gnfile = os.path.join(dirname, 'BUILD.gn')

    lines = open(gnfile).read().splitlines()
    matched = [i for i, l in enumerate(lines) if ' sources = [' in l]
    if skip_ambiguous and len(matched) > 1:
      print '[WARNING] Multiple sources in', gnfile
      continue

    if len(matched) < 1:
      continue
    print '  ', gnfile, rel
    index = matched[0]
    lines.insert(index + 1, '"%s",' % rel)
    open(gnfile, 'w').write('\n'.join(lines) + '\n')


def RemoveHeader(headers, skip_ambiguous=True):
  """Remove non-existing headers in GN files.

  When skip_ambiguous is True, skip if multiple matches are found.
  """
  edits = {}
  unhandled = []
  for filename in headers:
    filename = filename.strip()
    if not (filename.endswith('.h') or filename.endswith('.hh')):
      continue
    basename = os.path.basename(filename)
    print filename
    out, returncode = GitGrep('(/|")' + basename + '"')
    if returncode != 0 or not out:
      unhandled.append(filename)
      print '  Not found'
      continue

    grep_lines = out.splitlines()
    matches = []
    for line in grep_lines:
      gnfile, linenr, contents = line.split(':')
      print '    ', gnfile, linenr, contents
      linenr = int(linenr)
      lines = open(gnfile).read().splitlines()
      assert contents in lines[linenr - 1]
      matches.append((gnfile, linenr, contents))

    if len(matches) == 0:
      continue
    if len(matches) > 1:
      print '\n[WARNING] Ambiguous matching for', filename
      for i in enumerate(matches, 1):
        print '%d: %s' % (i[0], i[1])
      print
      if skip_ambiguous:
        continue

      picked = raw_input('Pick the matches ("2,3" for multiple): ')
      try:
        matches = [matches[int(i) - 1] for i in picked.split(',')]
      except (ValueError, IndexError):
        continue

    for match in matches:
      gnfile, linenr, contents = match
      print '  ', gnfile, linenr, contents
      edits.setdefault(gnfile, set()).add(linenr)

  for gnfile in edits:
    lines = open(gnfile).read().splitlines()
    for l in sorted(edits[gnfile], reverse=True):
      lines.pop(l - 1)
    open(gnfile, 'w').write('\n'.join(lines) + '\n')

  return unhandled


def main():
  parser = argparse.ArgumentParser()
  parser.add_argument('input_file', help="missing or non-existing headers, "
                      "output of check_gn_headers.py")
  parser.add_argument('--prefix',
                      help="only handle path name with this prefix")
  parser.add_argument('--remove', action='store_true',
                      help="treat input_file as non-existing headers")

  args, _extras = parser.parse_known_args()

  headers = open(args.input_file).readlines()

  if args.prefix:
    headers = [i for i in headers if i.startswith(args.prefix)]

  if args.remove:
    RemoveHeader(headers, False)
  else:
    unhandled = AddHeadersNextToCC(headers)
    AddHeadersToSources(unhandled)


if __name__ == '__main__':
  sys.exit(main())
