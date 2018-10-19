#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import sys
import util
import time
import subprocess

ADDR = "127.0.0.1:4544"
DURATION = "10s"


def deno_http_benchmark(deno_exe):
    deno_cmd = [deno_exe, "--allow-net", "tests/http_bench.ts", ADDR]
    print "http_benchmark testing DENO."
    return run(deno_cmd)


def node_http_benchmark(deno_exe):
    node_cmd = ["node", "tools/node_http.js", ADDR.split(":")[1]]
    print "http_benchmark testing NODE."
    return run(node_cmd)


def http_benchmark(deno_exe):
    deno_rps = deno_http_benchmark(deno_exe)
    node_rps = node_http_benchmark(deno_exe)

    return {"deno": deno_rps, "node": node_rps}


def run(server_cmd):
    # Run deno echo server in the background.
    server = subprocess.Popen(server_cmd)
    time.sleep(5)  # wait for server to wake up. TODO racy.
    wrk_platform = {
        "linux2": "linux",
        "darwin": "mac",
    }[sys.platform]
    try:
        cmd = "third_party/wrk/" + wrk_platform + "/wrk -d " + DURATION + " http://" + ADDR + "/"
        print cmd
        output = subprocess.check_output(cmd, shell=True)
        req_per_sec = util.parse_wrk_output(output)
        print output
        return req_per_sec
    finally:
        server.kill()


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/http_benchmark.py out/debug/deno"
        sys.exit(1)
    deno_http_benchmark(sys.argv[1])
