#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# Performs benchmark and append data to //website/data.json.
# If //website/data.json doesn't exist, this script tries to import it from
# gh-pages branch.
# To view the results locally run target/debug/test_server and visit
# http://localhost:4545/website

import os
import sys
import time
import subprocess
import util

MB = 1024 * 1024
SERVER_ADDR = "0.0.0.0:4544"
CLIENT_ADDR = "127.0.0.1 4544"


def cat(deno_exe, megs):
    size = megs * MB
    start = time.time()
    cmd = deno_exe + " run --allow-read "
    cmd += "cli/tests/cat.ts /dev/zero | head -c %s " % size
    print cmd
    subprocess.check_output(cmd, shell=True)
    end = time.time()
    return end - start


def tcp(deno_exe, megs):
    size = megs * MB
    # Run deno echo server in the background.
    args = [
        deno_exe, "run", "--allow-net", "cli/tests/echo_server.ts", SERVER_ADDR
    ]
    print args
    echo_server = subprocess.Popen(args)

    time.sleep(5)  # wait for deno to wake up. TODO racy.
    try:
        start = time.time()
        nc_cmd = "nc " + CLIENT_ADDR
        cmd = ("head -c %s /dev/zero " % size) + " | " + nc_cmd
        print cmd
        subprocess.check_output(cmd, shell=True)
        end = time.time()
        return end - start
    finally:
        echo_server.kill()


def main():
    deno_exe = sys.argv[1]
    megs = int(sys.argv[2])
    if not deno_exe or not megs:
        print "Usage ./tools/throughput_benchmark.py target/debug/deno 100"
        sys.exit(1)
    secs = tcp(sys.argv[1], megs)
    print secs, "seconds"


if __name__ == '__main__':
    main()
