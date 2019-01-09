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


def deno_net_http_benchmark(deno_exe):
    deno_cmd = [
        deno_exe, "--allow-net",
        "js/deps/https/deno.land/x/std/net/http_bench.ts", ADDR
    ]
    print "http_benchmark testing DENO using net/http."
    return run(
        deno_cmd,
        merge_env={
            # Load from //js/deps/https/deno.land/net/ submodule.
            "DENO_DIR": os.path.join(util.root_path, "js")
        })


def node_http_benchmark():
    node_cmd = ["node", "tools/node_http.js", ADDR.split(":")[1]]
    print "http_benchmark testing NODE."
    return run(node_cmd)


def node_tcp_benchmark():
    node_cmd = ["node", "tools/node_tcp.js", ADDR.split(":")[1]]
    print "http_benchmark testing node_tcp.js"
    return run(node_cmd)


def hyper_http_benchmark(hyper_hello_exe):
    hyper_cmd = [hyper_hello_exe, ADDR.split(":")[1]]
    print "http_benchmark testing RUST hyper."
    return run(hyper_cmd)


def http_benchmark(deno_exe, hyper_hello_exe):
    r = {}
    # TODO Rename to "deno_tcp"
    r["deno"] = deno_http_benchmark(deno_exe)
    r["deno_net_http"] = deno_net_http_benchmark(deno_exe)
    r["node"] = node_http_benchmark()
    r["node_tcp"] = node_tcp_benchmark()
    r["hyper"] = hyper_http_benchmark(hyper_hello_exe)
    return r


def run(server_cmd, merge_env=None):
    # Run deno echo server in the background.
    if merge_env is None:
        env = None
    else:
        env = os.environ.copy()
        for key, value in merge_env.iteritems():
            env[key] = value

    server = subprocess.Popen(server_cmd, env=env)
    time.sleep(5)  # wait for server to wake up. TODO racy.
    try:
        cmd = "third_party/wrk/%s/wrk -d %s http://%s/" % (util.platform(),
                                                           DURATION, ADDR)
        print cmd
        output = subprocess.check_output(cmd, shell=True)
        req_per_sec = util.parse_wrk_output(output)
        print output
        return req_per_sec
    finally:
        server.kill()


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/http_benchmark.py target/debug/deno"
        sys.exit(1)
    deno_net_http_benchmark(sys.argv[1])
