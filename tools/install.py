#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
from __future__ import print_function
import os
import json
import sys
import tempfile
import shutil
import gzip
from zipfile import ZipFile
import re
try:
    from urllib.request import urlopen
except ImportError:
    from urllib2 import urlopen

releases_url_html = "https://github.com/denoland/deno/releases/latest"
install_dir = os.path.join(tempfile.gettempdir(), "deno_install")
home = os.path.expanduser("~")

def get_latest_url():
    res = urlopen(releases_url_html)
    html = res.read().decode('utf-8')
    urls = re.findall(r'href=[\'"]?([^\'" >]+)', html)

    filename = {
        "darwin": "deno_osx_x64.gz",
        # python3 sys.platform returns linux ( python2 returns linux2 )
        "linux": "deno_linux_x64.gz",
        "linux2": "deno_linux_x64.gz",
        "win32": "deno_win_x64.zip",
        "cygwin": "deno_win_x64.zip"
    }[sys.platform]

    matching = [u for u in urls if filename in u]

    if len(matching) != 1:
        print("Bad download url")
        print("urls", urls)
        print("matching", matching)
        sys.exit(1)

    return "https://github.com" + matching[0]

        
def main():
    latest_url = get_latest_url()
    latest_fn = dlfile(latest_url)

    bin_dir = deno_bin_dir()
    exe_fn = os.path.join(bin_dir, "deno")

    if "zip" in latest_fn:
        with ZipFile(latest_fn, 'r') as z:
            with open(exe_fn, 'wb+') as exe:
                exe.write(z.read('deno.exe'))
    else:
        with gzip.open(latest_fn, 'rb') as f:
            content = f.read()
            with open(exe_fn, 'wb+') as exe:
                exe.write(content)

    os.chmod(exe_fn, 0o744)
    print("DENO_EXE: " + exe_fn)
    print("Now manually add %s to your $PATH" % bin_dir)
    print("Example:")
    print()
    print("  echo export PATH=\"%s\":\\$PATH >> $HOME/.bash_profile" % bin_dir)
    print()


def mkdir(d):
    if not os.path.exists(d):
        print("mkdir", d)
        os.mkdir(d)


def deno_bin_dir():
    install_dir = home
    d = os.path.join(install_dir, ".deno")
    b = os.path.join(d, "bin")
    mkdir(d)
    mkdir(b)
    return b


def dlfile(url):
    print("Downloading " + url)
    f = urlopen(url)
    mkdir(install_dir)
    p = os.path.join(install_dir, os.path.basename(url))
    print("Writing " + p)
    with open(p, "wb") as local_file:
        local_file.write(f.read())
    return p


if __name__ == '__main__':
    main()