import sys
from util import run
from third_party import tp, google_env


def download_v8_prebuilt():
    if sys.platform == 'win32':
        download_prebuilt("prebuilt/win/v8.lib.sha1")
        # TODO Ideally we wouldn't have to download both builds of V8.
        download_prebuilt("prebuilt/win/v8_debug.lib.sha1")
    elif sys.platform.startswith('linux'):
        download_prebuilt("prebuilt/linux64/libv8.a.sha1")
    elif sys.platform == 'darwin':
        download_prebuilt("prebuilt/mac/libv8.a.sha1")


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


def load():
    download_v8_prebuilt()


if __name__ == '__main__':
    sys.exit(load())
