# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os

# Utilities to read and write .jar.info files.
#
# A .jar.info file contains a simple mapping from fully-qualified Java class
# names to the source file that actually defines it.
#
# For APKs, the .jar.info maps the class names to the .jar file that which
# contains its .class definition instead.


def ParseJarInfoFile(info_path):
  """Parse a given .jar.info file as a dictionary.

  Args:
    info_path: input .jar.info file path.
  Returns:
    A new dictionary mapping fully-qualified Java class names to file paths.
  """
  info_data = dict()
  if os.path.exists(info_path):
    with open(info_path, 'r') as info_file:
      for line in info_file:
        line = line.strip()
        if line:
          fully_qualified_name, path = line.split(',', 1)
          info_data[fully_qualified_name] = path
  return info_data


def WriteJarInfoFile(info_path, info_data, source_file_map=None):
  """Generate a .jar.info file from a given dictionary.

  Args:
    info_path: output file path.
    info_data: a mapping of fully qualified Java class names to filepaths.
    source_file_map: an optional mapping from java source file paths to the
      corresponding source .srcjar. This is because info_data may contain the
      path of Java source files that where extracted from an .srcjar into a
      temporary location.
  """
  with open(info_path, 'w') as info_file:
    for fully_qualified_name, path in info_data.iteritems():
      if source_file_map and path in source_file_map:
        path = source_file_map[path]
        assert not path.startswith('/tmp'), (
            'Java file path should not be in temp dir: {}'.format(path))
      info_file.write('{},{}\n'.format(fully_qualified_name, path))
