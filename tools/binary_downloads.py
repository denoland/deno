#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# This is called from tools/setup.py in the case of a GN build or independently
# from tools/build_common.gn in the case of a cargo build.
import third_party
from util import (enable_ansi_colors, root_path)
import os
import sys
import prebuilt
import argparse


def binary_downloads():
    enable_ansi_colors()
    os.chdir(root_path)

    print "binary download"
    third_party.download_gn()
    third_party.download_clang_format()
    third_party.download_clang()
    third_party.maybe_download_sysroot()
    prebuilt.load_sccache()


if __name__ == '__main__':
    sys.exit(binary_downloads())
