#!/usr/bin/env python
#
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Deploys and executes a packaged Fuchsia executable on a target."""

import argparse
import logging
import sys

from common_args import AddCommonArgs, ConfigureLogging, \
                        GetDeploymentTargetForArgs
from run_package import RunPackage


def main():
  parser = argparse.ArgumentParser()
  AddCommonArgs(parser)
  parser.add_argument('child_args', nargs='*',
                      help='Arguments for the test process.')
  args = parser.parse_args()
  ConfigureLogging(args)

  with GetDeploymentTargetForArgs(args) as target:
    target.Start()
    return RunPackage(
        args.output_directory, target, args.package, args.package_name,
        args.package_dep, args.child_args, args.include_system_logs,
        args.install_only, args.package_manifest)


if __name__ == '__main__':
  sys.exit(main())
