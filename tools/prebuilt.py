# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import sys
import os
from util import run, root_path
from third_party import tp, google_env


def download_prebuilt(sha1_file):
    run([
        "python",
        tp('depot_tools/download_from_google_storage.py'),
        '--platform=' + sys.platform,
        '--no_auth',
        '--bucket=denoland',
        '--sha1_file',
        sha1_file,
    ],
        env=google_env())


def get_platform_path(tool):
    if sys.platform == 'win32':
        return "prebuilt/win/" + tool + ".exe"
    elif sys.platform.startswith('linux'):
        return "prebuilt/linux64/" + tool
    elif sys.platform == 'darwin':
        return "prebuilt/mac/" + tool


def load_sccache():
    p = get_platform_path("sccache")
    download_prebuilt(p + ".sha1")
    return os.path.join(root_path, p)


def load_hyperfine():
    p = get_platform_path("hyperfine")
    download_prebuilt(p + ".sha1")
    return os.path.join(root_path, p)
