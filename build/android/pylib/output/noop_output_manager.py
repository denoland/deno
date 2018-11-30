# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

from pylib.base import output_manager

# TODO(jbudorick): This class is currently mostly unused.
# Add a --bot-mode argument that all bots pass. If --bot-mode and
# --local-output args are both not passed to test runner then use this
# as the output manager impl.

# pylint: disable=no-self-use

class NoopOutputManager(output_manager.OutputManager):

  def __init__(self):
    super(NoopOutputManager, self).__init__()

  #override
  def _CreateArchivedFile(self, out_filename, out_subdir, datatype):
    del out_filename, out_subdir, datatype
    return NoopArchivedFile()


class NoopArchivedFile(output_manager.ArchivedFile):

  def __init__(self):
    super(NoopArchivedFile, self).__init__(None, None, None)

  def Link(self):
    """NoopArchivedFiles are not retained."""
    return ''

  def _Link(self):
    pass

  def Archive(self):
    """NoopArchivedFiles are not retained."""
    pass

  def _Archive(self):
    pass
