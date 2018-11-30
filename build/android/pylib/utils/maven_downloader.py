#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import errno
import logging
import os
import shutil

from devil.utils import cmd_helper
from devil.utils import parallelizer


def _MakeDirsIfAbsent(path):
  try:
    os.makedirs(path)
  except OSError as err:
    if err.errno != errno.EEXIST or not os.path.isdir(path):
      raise


class MavenDownloader(object):
  '''
  Downloads and installs the requested artifacts from the Google Maven repo.
  The artifacts are expected to be specified in the format
  "group_id:artifact_id:version:file_type", as the default file type is JAR
  but most Android libraries are provided as AARs, which would otherwise fail
  downloading. See Install()
  '''

  # Remote repository to download the artifacts from. The support library and
  # Google Play service are only distributed there, but third party libraries
  # could use Maven Central or JCenter for example. The default Maven remote
  # is Maven Central.
  _REMOTE_REPO = 'https://maven.google.com'

  # Default Maven repository.
  _DEFAULT_REPO_PATH = os.path.join(
      os.path.expanduser('~'), '.m2', 'repository')

  def __init__(self, debug=False):
    self._repo_path = MavenDownloader._DEFAULT_REPO_PATH
    self._remote_url = MavenDownloader._REMOTE_REPO
    self._debug = debug

  def Install(self, target_repo, artifacts, include_poms=False):
    logging.info('Installing %d artifacts...', len(artifacts))
    downloaders = [_SingleArtifactDownloader(self, artifact, target_repo)
                   for artifact in artifacts]
    if self._debug:
      for downloader in downloaders:
        downloader.Run(include_poms)
    else:
      parallelizer.SyncParallelizer(downloaders).Run(include_poms)
    logging.info('%d artifacts installed to %s', len(artifacts), target_repo)

  @property
  def repo_path(self):
    return self._repo_path

  @property
  def remote_url(self):
    return self._remote_url

  @property
  def debug(self):
    return self._debug


class _SingleArtifactDownloader(object):
  '''Handles downloading and installing a single Maven artifact.'''

  _POM_FILE_TYPE = 'pom'

  def __init__(self, download_manager, artifact, target_repo):
    self._download_manager = download_manager
    self._artifact = artifact
    self._target_repo = target_repo

  def Run(self, include_pom=False):
    parts = self._artifact.split(':')
    if len(parts) != 4:
      raise Exception('Artifacts expected as '
                      '"group_id:artifact_id:version:file_type".')
    group_id, artifact_id, version, file_type = parts
    self._InstallArtifact(group_id, artifact_id, version, file_type)

    if include_pom and file_type != _SingleArtifactDownloader._POM_FILE_TYPE:
      self._InstallArtifact(group_id, artifact_id, version,
                            _SingleArtifactDownloader._POM_FILE_TYPE)

  def _InstallArtifact(self, group_id, artifact_id, version, file_type):
    logging.debug('Processing %s', self._artifact)

    download_relpath = self._DownloadArtifact(
        group_id, artifact_id, version, file_type)
    logging.debug('Downloaded.')

    install_path = self._ImportArtifact(download_relpath)
    logging.debug('Installed %s', os.path.relpath(install_path))

  def _DownloadArtifact(self, group_id, artifact_id, version, file_type):
    '''
    Downloads the specified artifact using maven, to its standard location, see
    MavenDownloader._DEFAULT_REPO_PATH.
    '''
    cmd = ['mvn',
           'org.apache.maven.plugins:maven-dependency-plugin:RELEASE:get',
           '-DremoteRepositories={}'.format(self._download_manager.remote_url),
           '-Dartifact={}:{}:{}:{}'.format(group_id, artifact_id, version,
                                           file_type)]

    stdout = None if self._download_manager.debug else open(os.devnull, 'wb')

    try:
      ret_code = cmd_helper.Call(cmd, stdout=stdout)
      if ret_code != 0:
        raise Exception('Command "{}" failed'.format(' '.join(cmd)))
    except OSError as e:
      if e.errno == os.errno.ENOENT:
        raise Exception('mvn command not found. Please install Maven.')
      raise

    return os.path.join(os.path.join(*group_id.split('.')),
                        artifact_id,
                        version,
                        '{}-{}.{}'.format(artifact_id, version, file_type))

  def _ImportArtifact(self, artifact_path):
    src_dir = os.path.join(self._download_manager.repo_path, artifact_path)
    dst_dir = os.path.join(self._target_repo, os.path.dirname(artifact_path))

    _MakeDirsIfAbsent(dst_dir)
    shutil.copy(src_dir, dst_dir)

    return dst_dir
