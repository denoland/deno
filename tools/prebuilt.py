import sys
from util import run
from third_party import tp, google_env


def download_v8_prebuilt():
    if sys.platform == 'win32':
        sha1_file = "prebuilt/win/v8.lib.sha1"
    elif sys.platform.startswith('linux'):
        sha1_file = "prebuilt/linux64/libv8.a.sha1"
    elif sys.platform == 'darwin':
        sha1_file = "prebuilt/mac/libv8.a.sha1"

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


def load():
    download_v8_prebuilt()


if __name__ == '__main__':
    sys.exit(load())
