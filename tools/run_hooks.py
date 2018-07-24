#!/usr/bin/env python
import os
import sys
from util import run

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = os.path.join(root_path, "third_party")
depot_tools_path = os.path.join(third_party_path, "depot_tools")
os.chdir(root_path)


def download(filename):
    run([
        "python",
        os.path.join(depot_tools_path + '/download_from_google_storage.py'),
        '--platform=' + sys.platform, '--no_auth', '--bucket=chromium-gn',
        '--sha1_file',
        os.path.join(root_path, filename)
    ])


if sys.platform == 'win32':
    download("third_party/v8/buildtools/win/gn.exe.sha1")
elif sys.platform == 'darwin':
    download("third_party/v8/buildtools/mac/gn.sha1")
elif sys.platform.startswith('linux'):
    download("third_party/v8/buildtools/linux64/gn.sha1")

run(['python', 'third_party/v8/tools/clang/scripts/update.py', '--if-needed'])
