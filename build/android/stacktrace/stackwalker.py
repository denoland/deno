#!/usr/bin/env python
#
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import os
import re
import sys
import tempfile

if __name__ == '__main__':
  sys.path.append(os.path.join(os.path.dirname(__file__), '..'))
from pylib.constants import host_paths

if host_paths.DEVIL_PATH not in sys.path:
  sys.path.append(host_paths.DEVIL_PATH)
from devil.utils import cmd_helper


_MICRODUMP_BEGIN = re.compile(
    '.*google-breakpad: -----BEGIN BREAKPAD MICRODUMP-----')
_MICRODUMP_END = re.compile(
    '.*google-breakpad: -----END BREAKPAD MICRODUMP-----')

""" Example Microdump
<timestamp>  6270  6131 F google-breakpad: -----BEGIN BREAKPAD MICRODUMP-----
<timestamp>  6270  6131 F google-breakpad: V Chrome_Android:54.0.2790.0
...
<timestamp>  6270  6131 F google-breakpad: -----END BREAKPAD MICRODUMP-----

"""


def GetMicroDumps(dump_path):
  """Returns all microdumps found in given log file

  Args:
    dump_path: Path to the log file.

  Returns:
    List of all microdumps as lists of lines.
  """
  with open(dump_path, 'r') as d:
    data = d.read()
  all_dumps = []
  current_dump = None
  for line in data.splitlines():
    if current_dump is not None:
      if _MICRODUMP_END.match(line):
        current_dump.append(line)
        all_dumps.append(current_dump)
        current_dump = None
      else:
        current_dump.append(line)
    elif _MICRODUMP_BEGIN.match(line):
      current_dump = []
      current_dump.append(line)
  return all_dumps


def SymbolizeMicroDump(stackwalker_binary_path, dump, symbols_path):
  """Runs stackwalker on microdump.

  Runs the stackwalker binary at stackwalker_binary_path on a given microdump
  using the symbols at symbols_path.

  Args:
    stackwalker_binary_path: Path to the stackwalker binary.
    dump: The microdump to run the stackwalker on.
    symbols_path: Path the the symbols file to use.

  Returns:
    Output from stackwalker tool.
  """
  with tempfile.NamedTemporaryFile() as tf:
    for l in dump:
      tf.write('%s\n' % l)
    cmd = [stackwalker_binary_path, tf.name, symbols_path]
    return cmd_helper.GetCmdOutput(cmd)


def AddArguments(parser):
  parser.add_argument('--stackwalker-binary-path', required=True,
                      help='Path to stackwalker binary.')
  parser.add_argument('--stack-trace-path', required=True,
                      help='Path to stacktrace containing microdump.')
  parser.add_argument('--symbols-path', required=True,
                      help='Path to symbols file.')
  parser.add_argument('--output-file',
                      help='Path to dump stacktrace output to')


def _PrintAndLog(line, fp):
  if fp:
    fp.write('%s\n' % line)
  print line


def main():
  parser = argparse.ArgumentParser()
  AddArguments(parser)
  args = parser.parse_args()

  micro_dumps = GetMicroDumps(args.stack_trace_path)
  if not micro_dumps:
    print 'No microdump found. Exiting.'
    return 0

  symbolized_dumps = []
  for micro_dump in micro_dumps:
    symbolized_dumps.append(SymbolizeMicroDump(
        args.stackwalker_binary_path, micro_dump, args.symbols_path))

  try:
    fp = open(args.output_file, 'w') if args.output_file else None
    _PrintAndLog('%d microdumps found.' % len(micro_dumps), fp)
    _PrintAndLog('---------- Start output from stackwalker ----------', fp)
    for index, symbolized_dump in list(enumerate(symbolized_dumps)):
      _PrintAndLog(
          '------------------ Start dump %d ------------------' % index, fp)
      _PrintAndLog(symbolized_dump, fp)
      _PrintAndLog(
          '------------------- End dump %d -------------------' % index, fp)
    _PrintAndLog('----------- End output from stackwalker -----------', fp)
  except Exception:
    if fp:
      fp.close()
    raise
  return 0


if __name__ == '__main__':
  sys.exit(main())
