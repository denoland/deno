#!/usr/bin/env python

import os
import sys
import util
import time
import subprocess

ADDR = "127.0.0.1:4544"
DURATION = "10s"


def http_benchmark(deno_exe):
    deno_cmd = [deno_exe, "--allow-net", "tests/http_bench.ts", ADDR]
    node_cmd = ["node", "tools/node_http.js", ADDR.split(":")[1]]

    print "http_benchmark testing DENO."
    deno_rps = run(deno_cmd)

    print "http_benchmark testing NODE."
    node_rps = run(node_cmd)

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
        print "Usage ./tools/tcp_http_benchmark.py out/debug/deno"
        sys.exit(1)
    http_benchmark(sys.argv[1])
