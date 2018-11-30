# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import hashlib
import os

from pylib.base import output_manager
from pylib.output import noop_output_manager
from pylib.utils import logdog_helper
from pylib.utils import google_storage_helper


class RemoteOutputManager(output_manager.OutputManager):

  def __init__(self, bucket):
    """Uploads output files to Google Storage or LogDog.

    Files will either be uploaded directly to Google Storage or LogDog
    depending on the datatype.

    Args
      bucket: Bucket to use when saving to Google Storage.
    """
    super(RemoteOutputManager, self).__init__()
    self._bucket = bucket

  #override
  def _CreateArchivedFile(self, out_filename, out_subdir, datatype):
    if datatype == output_manager.Datatype.TEXT:
      try:
        logdog_helper.get_logdog_client()
        return LogdogArchivedFile(out_filename, out_subdir, datatype)
      except RuntimeError:
        return noop_output_manager.NoopArchivedFile()
    else:
      if self._bucket is None:
        return noop_output_manager.NoopArchivedFile()
      return GoogleStorageArchivedFile(
          out_filename, out_subdir, datatype, self._bucket)


class LogdogArchivedFile(output_manager.ArchivedFile):

  def __init__(self, out_filename, out_subdir, datatype):
    super(LogdogArchivedFile, self).__init__(out_filename, out_subdir, datatype)
    self._stream_name = '%s_%s' % (out_subdir, out_filename)

  def _Link(self):
    return logdog_helper.get_viewer_url(self._stream_name)

  def _Archive(self):
    with open(self.name, 'r') as f:
      logdog_helper.text(self._stream_name, f.read())


class GoogleStorageArchivedFile(output_manager.ArchivedFile):

  def __init__(self, out_filename, out_subdir, datatype, bucket):
    super(GoogleStorageArchivedFile, self).__init__(
        out_filename, out_subdir, datatype)
    self._bucket = bucket
    self._upload_path = None
    self._content_addressed = None

  def _PrepareArchive(self):
    self._content_addressed = (self._datatype in (
        output_manager.Datatype.HTML,
        output_manager.Datatype.PNG,
        output_manager.Datatype.JSON))
    if self._content_addressed:
      sha1 = hashlib.sha1()
      with open(self.name, 'rb') as f:
        sha1.update(f.read())
      self._upload_path = sha1.hexdigest()
    else:
      self._upload_path = os.path.join(self._out_subdir, self._out_filename)

  def _Link(self):
    return google_storage_helper.get_url_link(
        self._upload_path, self._bucket)

  def _Archive(self):
    if (self._content_addressed and
        google_storage_helper.exists(self._upload_path, self._bucket)):
      return

    google_storage_helper.upload(
        self._upload_path, self.name, self._bucket, content_type=self._datatype)
