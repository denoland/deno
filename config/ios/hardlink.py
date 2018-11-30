# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Recursively create hardlink to target named output."""


import argparse
import os
import shutil


def CreateHardlinkHelper(target, output):
  """Recursively create a hardlink named output pointing to target.

  Args:
    target: path to an existing file or directory
    output: path to the newly created hardlink

  This function assumes that output does not exists but that the parent
  directory containing output does. If those conditions are false, then
  the function will fails with an exception corresponding to an OS error.
  """
  if os.path.islink(target):
    os.symlink(os.readlink(target), output)
  elif not os.path.isdir(target):
    try:
      os.link(target, output)
    except:
      shutil.copy(target, output)
  else:
    os.mkdir(output)
    for name in os.listdir(target):
      CreateHardlinkHelper(
          os.path.join(target, name),
          os.path.join(output, name))


def CreateHardlink(target, output):
  """Recursively create a hardlink named output pointing to target.

  Args:
    target: path to an existing file or directory
    output: path to the newly created hardlink

  If output already exists, it is first removed. In all cases, the
  parent directory containing output is created.
  """
  if os.path.exists(output):
    shutil.rmtree(output)

  parent_dir = os.path.dirname(os.path.abspath(output))
  if not os.path.isdir(parent_dir):
    os.makedirs(parent_dir)

  CreateHardlinkHelper(target, output)


def Main():
  parser = argparse.ArgumentParser()
  parser.add_argument('target', help='path to the file or directory to link to')
  parser.add_argument('output', help='name of the hardlink to create')
  args = parser.parse_args()

  CreateHardlink(args.target, args.output)


if __name__ == '__main__':
  Main()
