#!/usr/bin/env python
#
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

'''Prepares the Google Play services split client libraries before usage by
Chrome's build system.

We need to preprocess Google Play services before using it in Chrome builds
mostly to remove unused resources (unsupported languages, unused drawables,
etc.) as proper resource shrinking is not yet supported by our build system.
(See https://crbug.com/636448)

The script is meant to be used with an unpacked library repository. One can
be obtained by downloading the "extra-google-m2repository" from the Android SDK
Manager and extracting the AARs from the desired version as the following
structure:

    REPOSITORY_DIR
    +-- CLIENT_1
    |   +-- <content of the first AAR file>
    +-- CLIENT_2
    +-- etc.

The output will follow the same structure, with fewer resource files, in the
provided output directory.
'''

import argparse
import glob
import itertools
import os
import shutil
import stat
import sys
import tempfile
import textwrap
import zipfile

sys.path.append(os.path.join(os.path.dirname(__file__), os.pardir))
from play_services import utils
from pylib.utils import argparse_utils


def main():
  parser = argparse.ArgumentParser(description=(
      "Prepares the Google Play services split client libraries before usage "
      "by Chrome's build system. See the script's documentation for more a "
      "detailed help."))
  argparse_utils.CustomHelpAction.EnableFor(parser)
  required_args = parser.add_argument_group('required named arguments')
  required_args.add_argument('-r',
                             '--repository',
                             help=('the Google Play services repository '
                                   'location'),
                             required=True,
                             metavar='FILE')
  required_args.add_argument('-d',
                             '--root-dir',
                             help='the directory which GN considers the root',
                             required=True,
                             metavar='FILE')
  required_args.add_argument('-o',
                             '--out-dir',
                             help='the output directory',
                             required=True,
                             metavar='FILE')
  required_args.add_argument('-g',
                             '--gni-out-file',
                             help='the GN output file',
                             required=True,
                             metavar='FILE')
  required_args.add_argument('-c',
                             '--config-file',
                             help='the config file path',
                             required=True,
                             metavar='FILE')
  parser.add_argument('--config-help',
                      action='custom_help',
                      custom_help_text=utils.ConfigParser.__doc__,
                      help='show the configuration file format help')

  args = parser.parse_args()

  return ProcessGooglePlayServices(args.repository,
                                   args.root_dir,
                                   args.out_dir,
                                   args.gni_out_file,
                                   args.config_file)


def ProcessGooglePlayServices(
    repo, root_dir, out_dir, gni_out_file, config_path):
  config = utils.ConfigParser(config_path)

  tmp_root = tempfile.mkdtemp()
  try:
    tmp_paths = _SetupTempDir(tmp_root)
    _ImportFromExtractedRepo(config, tmp_paths, repo)
    _ProcessResources(config, tmp_paths, repo)
    _CopyToOutput(tmp_paths, out_dir)
    _EnumerateProguardFiles(root_dir, out_dir, gni_out_file)
    _UpdateVersionInConfig(config, tmp_paths)
  finally:
    shutil.rmtree(tmp_root)

  return 0


def _SetupTempDir(tmp_root):
  tmp_paths = {
      'root': tmp_root,
      'imported_clients': os.path.join(tmp_root, 'imported_clients'),
      'extracted_jars': os.path.join(tmp_root, 'jar'),
      'combined_jar': os.path.join(tmp_root, 'google-play-services.jar'),
  }
  os.mkdir(tmp_paths['imported_clients'])
  os.mkdir(tmp_paths['extracted_jars'])

  return tmp_paths


def _MakeWritable(dir_path):
  for root, dirs, files in os.walk(dir_path):
    for path in itertools.chain(dirs, files):
      st = os.stat(os.path.join(root, path))
      os.chmod(os.path.join(root, path), st.st_mode | stat.S_IWUSR)


# E.g. turn "base_1p" into "base"
def _RemovePartySuffix(client):
  return client[:-3] if client[-3:] == '_1p' else client


def _ImportFromExtractedRepo(config, tmp_paths, repo):
  # Import the clients
  try:
    for client in config.clients:
      client_out_dir = os.path.join(tmp_paths['imported_clients'], client)
      shutil.copytree(os.path.join(repo, client), client_out_dir)
  finally:
    _MakeWritable(tmp_paths['imported_clients'])


def _ProcessResources(config, tmp_paths, repo):
  LOCALIZED_VALUES_BASE_NAME = 'values-'
  locale_whitelist = set(config.locale_whitelist)

  # The directory structure here is:
  # <imported_clients temp dir>/<client name>_1p/res/<res type>/<res file>.xml
  for client_dir in os.listdir(tmp_paths['imported_clients']):
    client_prefix = _RemovePartySuffix(client_dir) + '_'

    res_path = os.path.join(tmp_paths['imported_clients'], client_dir, 'res')
    if not os.path.isdir(res_path):
      continue

    for res_type in os.listdir(res_path):
      res_type_path = os.path.join(res_path, res_type)

      if res_type.startswith('drawable'):
        shutil.rmtree(res_type_path)
        continue

      if res_type.startswith(LOCALIZED_VALUES_BASE_NAME):
        dir_locale = res_type[len(LOCALIZED_VALUES_BASE_NAME):]
        if dir_locale not in locale_whitelist:
          shutil.rmtree(res_type_path)
          continue

      if res_type.startswith('values'):
        # Beginning with v3, resource file names are not necessarily unique,
        # and would overwrite each other when merged at build time. Prefix each
        # "values" resource file with its client name.
        for res_file in os.listdir(res_type_path):
          os.rename(os.path.join(res_type_path, res_file),
                    os.path.join(res_type_path, client_prefix + res_file))

  # Reimport files from the whitelist.
  for res_path in config.resource_whitelist:
    for whitelisted_file in glob.glob(os.path.join(repo, res_path)):
      resolved_file = os.path.relpath(whitelisted_file, repo)
      rebased_res = os.path.join(tmp_paths['imported_clients'], resolved_file)

      if not os.path.exists(os.path.dirname(rebased_res)):
        os.makedirs(os.path.dirname(rebased_res))

      try:
        shutil.copy(os.path.join(repo, whitelisted_file), rebased_res)
      finally:
        _MakeWritable(rebased_res)


def _CopyToOutput(tmp_paths, out_dir):
  shutil.rmtree(out_dir, ignore_errors=True)
  shutil.copytree(tmp_paths['imported_clients'], out_dir)


# Write a GN file containing a list of each GMS client's proguard file (if any).
def _EnumerateProguardFiles(root_dir, out_dir, gni_path):
  gni_dir = os.path.dirname(gni_path)
  gni_template = textwrap.dedent('''\
      # Copyright 2017 The Chromium Authors. All rights reserved.
      # Use of this source code is governed by a BSD-style license that can be
      # found in the LICENSE file.

      # This file generated by {script}
      gms_proguard_configs = [
      {body}
      ]
      ''')

  gni_lines = []
  for client_dir in os.listdir(out_dir):
    proguard_path = os.path.join(
        out_dir, client_dir, 'proguard.txt')
    if os.path.exists(proguard_path):
      rooted_path = os.path.relpath(proguard_path, root_dir)
      gni_lines.append('  "//{}",'.format(rooted_path))
  gni_lines.sort()

  gni_text = gni_template.format(
      script=os.path.relpath(sys.argv[0], gni_dir),
      body='\n'.join(gni_lines))

  with open(gni_path, 'w') as gni_file:
    gni_file.write(gni_text)


def _UpdateVersionInConfig(config, tmp_paths):
  version_xml_path = os.path.join(tmp_paths['imported_clients'],
                                  config.version_xml_path)
  play_services_full_version = utils.GetVersionNumberFromLibraryResources(
      version_xml_path)
  config.UpdateVersionNumber(play_services_full_version)


def _ExtractAll(zip_path, out_path):
  with zipfile.ZipFile(zip_path, 'r') as zip_file:
    zip_file.extractall(out_path)

if __name__ == '__main__':
  sys.exit(main())
