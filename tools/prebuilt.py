# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import sys
import os
from util import run, root_path
from third_party import tp, google_env


# TODO: bucket argument can be removed when artifacts
# are uploaded to denoland bucket
def download_prebuilt(bucket, sha1_file, extra_args=None):
    if extra_args is None:
        extra_args = []
    run([
        "python",
        tp('depot_tools/download_from_google_storage.py'),
        '--platform=' + sys.platform,
        '--no_auth',
        '--bucket=' + bucket,
        '--sha1_file',
        sha1_file,
    ] + extra_args,
        env=google_env())


def load_sccache():
    if sys.platform == 'win32':
        p = "prebuilt/win/sccache.exe"
    elif sys.platform.startswith('linux'):
        p = "prebuilt/linux64/sccache"
    elif sys.platform == 'darwin':
        p = "prebuilt/mac/sccache"
    download_prebuilt("denoland", p + ".sha1")
    return os.path.join(root_path, p)


def load_hyperfine():
    if sys.platform == 'win32':
        download_prebuilt("denoland", "prebuilt/win/hyperfine.exe.sha1")
    elif sys.platform.startswith('linux'):
        download_prebuilt("denoland", "prebuilt/linux64/hyperfine.sha1")
    elif sys.platform == 'darwin':
        download_prebuilt("denoland", "prebuilt/mac/hyperfine.sha1")


def load_rust():
    if sys.platform == 'win32':
        download_prebuilt("deno-rust", "prebuilt/win/rust.tar.gz.sha1", ["-u"])
    elif sys.platform.startswith('linux'):
        download_prebuilt("deno-rust", "prebuilt/linux64/rust.tar.gz.sha1",
                          ["-u"])
    elif sys.platform == 'darwin':
        download_prebuilt("deno-rust", "prebuilt/mac/rust.tar.gz.sha1", ["-u"])
