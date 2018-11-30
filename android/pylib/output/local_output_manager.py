# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import time
import os
import shutil
import urllib

from pylib.base import output_manager


class LocalOutputManager(output_manager.OutputManager):
  """Saves and manages test output files locally in output directory.

  Location files will be saved in {output_dir}/TEST_RESULTS_{timestamp}.
  """

  def __init__(self, output_dir):
    super(LocalOutputManager, self).__init__()
    timestamp = time.strftime(
        '%Y_%m_%dT%H_%M_%S', time.localtime())
    self._output_root = os.path.abspath(os.path.join(
        output_dir, 'TEST_RESULTS_%s' % timestamp))

  #override
  def _CreateArchivedFile(self, out_filename, out_subdir, datatype):
    return LocalArchivedFile(
        out_filename, out_subdir, datatype, self._output_root)


class LocalArchivedFile(output_manager.ArchivedFile):

  def __init__(self, out_filename, out_subdir, datatype, out_root):
    super(LocalArchivedFile, self).__init__(
        out_filename, out_subdir, datatype)
    self._output_path = os.path.join(out_root, out_subdir, out_filename)

  def _Link(self):
    return 'file://%s' % urllib.quote(self._output_path)

  def _Archive(self):
    if not os.path.exists(os.path.dirname(self._output_path)):
      os.makedirs(os.path.dirname(self._output_path))
    shutil.copy(self.name, self._output_path)
