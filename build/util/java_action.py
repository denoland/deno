#!/usr/bin/env python
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Wrapper script to run java command as action with gn."""

import os
import subprocess
import sys

EXIT_SUCCESS = 0
EXIT_FAILURE = 1


def IsExecutable(path):
  """Returns whether file at |path| exists and is executable.

  Args:
    path: absolute or relative path to test.

  Returns:
    True if the file at |path| exists, False otherwise.
  """
  return os.path.isfile(path) and os.access(path, os.X_OK)


def FindCommand(command):
  """Looks up for |command| in PATH.

  Args:
    command: name of the command to lookup, if command is a relative or
      absolute path (i.e. contains some path separator) then only that
      path will be tested.

  Returns:
    Full path to command or None if the command was not found.

    On Windows, this respects the PATHEXT environment variable when the
    command name does not have an extension.
  """
  fpath, _ = os.path.split(command)
  if fpath:
    if IsExecutable(command):
      return command

  if sys.platform == 'win32':
    # On Windows, if the command does not have an extension, cmd.exe will
    # try all extensions from PATHEXT when resolving the full path.
    command, ext = os.path.splitext(command)
    if not ext:
      exts = os.environ['PATHEXT'].split(os.path.pathsep)
    else:
      exts = [ext]
  else:
    exts = ['']

  for path in os.environ['PATH'].split(os.path.pathsep):
    for ext in exts:
      path = os.path.join(path, command) + ext
      if IsExecutable(path):
        return path

  return None


def main():
  java_path = FindCommand('java')
  if not java_path:
    sys.stderr.write('java: command not found\n')
    sys.exit(EXIT_FAILURE)

  args = sys.argv[1:]
  if len(args) < 2 or args[0] != '-jar':
    sys.stderr.write('usage: %s -jar JARPATH [java_args]...\n' % sys.argv[0])
    sys.exit(EXIT_FAILURE)

  return subprocess.check_call([java_path] + args)


if __name__ == '__main__':
  sys.exit(main())
