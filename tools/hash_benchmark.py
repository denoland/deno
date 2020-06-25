#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# Performs benchmark on hash algorithms

import os
import sys
import time
import subprocess

algorithms = [
    "md5",
    "sha1",
    "sha224",
    "sha256",
    "sha512",
    "sha3-224",
    "sha3-256",
    "sha3-384",
    "sha3-512",
]


def run_benchmark(deno_exe, method, input_file):
    # compile
    subprocess.call([deno_exe, "run", "cli/tests/hash.ts"])

    for alg in algorithms:
        args = [
            deno_exe, "run", "--allow-read", "cli/tests/hash.ts", method, alg,
            input_file
        ]

        p = subprocess.Popen(args, stdout=subprocess.PIPE)
        (out, _) = p.communicate()

        elapsed = out.split(':')[1].strip()
        print "[{}] {}".format(alg, elapsed)


def main():
    if len(sys.argv) < 4:
        print "Usage ./tools/hash_benchmark.py path/to/deno method input"
        sys.exit(1)

    run_benchmark(sys.argv[1], sys.argv[2], sys.argv[3])


if __name__ == '__main__':
    main()
