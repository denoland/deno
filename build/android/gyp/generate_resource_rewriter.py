#!/usr/bin/env python
#
# Copyright (c) 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Generate ResourceRewriter.java which overwrites the given package's
   resource id.
"""

import argparse
import os
import sys
import zipfile

from util import build_utils

# Import jinja2 from third_party/jinja2
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__),
                                             '..',
                                             '..',
                                             '..',
                                             'third_party')))
import jinja2


RESOURCE_REWRITER_JAVA="ResourceRewriter.java"

RESOURCE_REWRITER="""/* AUTO-GENERATED FILE.  DO NOT MODIFY. */

package {{ package }};
/**
 * Helper class used to fix up resource ids.
 */
class ResourceRewriter {
    /**
     * Rewrite the R 'constants' for the WebView.
     */
    public static void rewriteRValues(final int packageId) {
        {% for res_package in res_packages %}
        {{ res_package }}.R.onResourcesLoaded(packageId);
        {% endfor %}
    }
}
"""

def ParseArgs(args):
  """Parses command line options.

  Returns:
    An Namespace from argparse.parse_args()
  """
  parser = argparse.ArgumentParser(prog='generate_resource_rewriter')

  parser.add_argument('--package-name',
                      required=True,
                      help='The package name of ResourceRewriter.')
  parser.add_argument('--dep-packages',
                      required=True,
                      help='A list of packages whose resource id will be'
                           'overwritten in ResourceRewriter.')
  parser.add_argument('--output-dir',
                      help='A output directory of generated'
                           ' ResourceRewriter.java')
  parser.add_argument('--srcjar',
                      help='The path of generated srcjar which has'
                           ' ResourceRewriter.java')

  return parser.parse_args(args)


def CreateResourceRewriter(package, res_packages, output_dir):
  build_utils.MakeDirectory(output_dir)
  java_path = os.path.join(output_dir, RESOURCE_REWRITER_JAVA)
  template = jinja2.Template(RESOURCE_REWRITER,
                             trim_blocks=True,
                             lstrip_blocks=True)
  output = template.render(package=package, res_packages=res_packages)
  with open(java_path, 'w') as f:
    f.write(output)

def CreateResourceRewriterSrcjar(package, res_packages, srcjar_path):
  with build_utils.TempDir() as temp_dir:
    output_dir = os.path.join(temp_dir, *package.split('.'))
    CreateResourceRewriter(package, res_packages, output_dir)
    build_utils.DoZip([os.path.join(output_dir, RESOURCE_REWRITER_JAVA)],
                      srcjar_path,
                      temp_dir)


def main():
  options = ParseArgs(build_utils.ExpandFileArgs(sys.argv[1:]))
  package = options.package_name
  if options.output_dir:
    output_dir = os.path.join(options.output_dir, *package.split('.'))
    CreateResourceRewriter(
        package,
        build_utils.ParseGnList(options.dep_packages),
        output_dir)
  else:
    CreateResourceRewriterSrcjar(
        package,
        build_utils.ParseGnList(options.dep_packages),
        options.srcjar)

  return 0

if __name__ == '__main__':
  sys.exit(main())
