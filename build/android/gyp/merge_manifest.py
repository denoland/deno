#!/usr/bin/env python

# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Merges dependency Android manifests into a root manifest."""

import argparse
import contextlib
import os
import sys
import tempfile
import xml.dom.minidom as minidom

from util import build_utils

# Tools library directory - relative to Android SDK root
SDK_TOOLS_LIB_DIR = os.path.join('tools', 'lib')

MANIFEST_MERGER_MAIN_CLASS = 'com.android.manifmerger.Merger'
MANIFEST_MERGER_JARS = [
  'common{suffix}.jar',
  'manifest-merger{suffix}.jar',
  'sdk-common{suffix}.jar',
  'sdklib{suffix}.jar',
]

TOOLS_NAMESPACE_PREFIX = 'tools'
TOOLS_NAMESPACE = 'http://schemas.android.com/tools'


@contextlib.contextmanager
def _ProcessManifest(manifest_path):
  """Patches an Android manifest to always include the 'tools' namespace
  declaration, as it is not propagated by the manifest merger from the SDK.

  See https://issuetracker.google.com/issues/63411481
  """
  doc = minidom.parse(manifest_path)
  manifests = doc.getElementsByTagName('manifest')
  assert len(manifests) == 1
  manifest = manifests[0]
  package = manifest.getAttribute('package')

  manifest.setAttribute('xmlns:%s' % TOOLS_NAMESPACE_PREFIX, TOOLS_NAMESPACE)

  tmp_prefix = os.path.basename(manifest_path)
  with tempfile.NamedTemporaryFile(prefix=tmp_prefix) as patched_manifest:
    doc.writexml(patched_manifest)
    patched_manifest.flush()
    yield patched_manifest.name, package


def _BuildManifestMergerClasspath(build_vars):
  return ':'.join([
    os.path.join(
      build_vars['android_sdk_root'],
      SDK_TOOLS_LIB_DIR,
      jar.format(suffix=build_vars['android_sdk_tools_version_suffix']))
    for jar in MANIFEST_MERGER_JARS
  ])


def main(argv):
  argv = build_utils.ExpandFileArgs(argv)
  parser = argparse.ArgumentParser(description=__doc__)
  build_utils.AddDepfileOption(parser)
  parser.add_argument('--build-vars',
                      help='Path to GN build vars file',
                      required=True)
  parser.add_argument('--root-manifest',
                      help='Root manifest which to merge into',
                      required=True)
  parser.add_argument('--output', help='Output manifest path', required=True)
  parser.add_argument('--extras',
                      help='GN list of additional manifest to merge')
  args = parser.parse_args(argv)

  classpath = _BuildManifestMergerClasspath(
      build_utils.ReadBuildVars(args.build_vars))

  with build_utils.AtomicOutput(args.output) as f:
    cmd = [
      'java',
      '-cp',
      classpath,
      MANIFEST_MERGER_MAIN_CLASS,
      '--out', f.name,
    ]

    extras = build_utils.ParseGnList(args.extras)
    if extras:
      cmd += ['--libs', ':'.join(extras)]

    with _ProcessManifest(args.root_manifest) as tup:
      root_manifest, package = tup
      cmd += ['--main', root_manifest, '--property', 'PACKAGE=' + package]
      build_utils.CheckOutput(cmd,
        # https://issuetracker.google.com/issues/63514300:
        # The merger doesn't set a nonzero exit code for failures.
        fail_func=lambda returncode, stderr: returncode != 0 or
          build_utils.IsTimeStale(f.name, [root_manifest] + extras))
  if args.depfile:
    inputs = extras + classpath.split(':')
    build_utils.WriteDepfile(args.depfile, args.output, inputs=inputs,
                             add_pydeps=False)


if __name__ == '__main__':
  main(sys.argv[1:])
