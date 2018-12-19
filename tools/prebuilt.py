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


def load_sccache():
    if sys.platform == 'win32':
        p = "prebuilt/win/sccache.exe"
    elif sys.platform.startswith('linux'):
        p = "prebuilt/linux64/sccache"
    elif sys.platform == 'darwin':
        p = "prebuilt/mac/sccache"
    download_prebuilt(p + ".sha1")
    return os.path.join(root_path, p)


def load_hyperfine():
    if sys.platform == 'win32':
        download_prebuilt("prebuilt/win/hyperfine.exe.sha1")
    elif sys.platform.startswith('linux'):
        download_prebuilt("prebuilt/linux64/hyperfine.sha1")
    elif sys.platform == 'darwin':
        download_prebuilt("prebuilt/mac/hyperfine.sha1")
