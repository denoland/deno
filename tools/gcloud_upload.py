#!/usr/bin/env python
# Prereq:
# - gcloud auth login
# - gcloud config set project deno-223616
# This program uploads the specified file to GCloud Storage in the denoland
# bucket. It places a checksum file into the current directory using the base
# filename of the specified file.

import os
import sys
import hashlib
from util import root_path, run
from third_party import tp


def print_usage():
    print "Usage: ./tools/gcloud_upload.py target/release/obj/third_party/v8/libv8.a"
    sys.exit(1)


def compute_sha1(filename):
    m = hashlib.sha1()
    with open(filename) as f:
        m.update(f.read())
    return m.hexdigest()


def main(argv):
    if len(argv) != 2:
        print_usage()
    os.chdir(root_path)

    filename = sys.argv[1]
    basename = os.path.basename(filename)

    sha1 = compute_sha1(filename)
    print sha1

    gs_url = "gs://denoland/" + sha1

    #gsutil = tp("depot_tools/gsutil.py")
    gsutil = "gsutil"  # standalone installation

    run([gsutil, "cp", filename, gs_url])
    run([gsutil, "acl", "ch", "-u", "AllUsers:R", gs_url])

    target_filename = basename + ".sha1"
    with open(target_filename, 'w') as f:
        f.write(sha1)
    print "Wrote", target_filename


if __name__ == '__main__':
    sys.exit(main(sys.argv))
