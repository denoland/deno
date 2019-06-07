#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import sys
import util
import time
import subprocess

# Some of the benchmarks in this file have been renamed. In case the history
# somehow gets messed up:
#   "node_http" was once called "node"
#   "deno_tcp" was once called "deno"
#   "deno_http" was once called "deno_net_http"

ADDR = "127.0.0.1:4544"
ORIGIN_ADDR = "127.0.0.1:4545"
DURATION = "10s"


def deno_tcp(deno_exe):
    deno_cmd = [deno_exe, "run", "--allow-net", "tools/deno_tcp.ts", ADDR]
    print "http_benchmark testing DENO."
    return run(deno_cmd)


def deno_http(deno_exe):
    deno_cmd = [
        deno_exe, "run", "--allow-net",
        "js/deps/https/deno.land/std/http/http_bench.ts", ADDR
    ]
    print "http_benchmark testing DENO using net/http."
    return run(
        deno_cmd,
        merge_env={
            # Load from //js/deps/https/deno.land/net/ submodule.
            "DENO_DIR": os.path.join(util.root_path, "js")
        })


def deno_tcp_proxy(deno_exe, hyper_hello_exe):
    deno_cmd = [
        deno_exe, "run", "--allow-net", "tools/deno_tcp_proxy.ts", ADDR,
        ORIGIN_ADDR
    ]
    print "http_proxy_benchmark testing DENO using net/tcp."
    return run(
        deno_cmd,
        merge_env={"DENO_DIR": os.path.join(util.root_path, "js")},
        origin_cmd=http_proxy_origin(hyper_hello_exe))


def deno_http_proxy(deno_exe, hyper_hello_exe):
    deno_cmd = [
        deno_exe, "run", "--allow-net", "tools/deno_http_proxy.ts", ADDR,
        ORIGIN_ADDR
    ]
    print "http_proxy_benchmark testing DENO using net/http."
    return run(
        deno_cmd,
        merge_env={"DENO_DIR": os.path.join(util.root_path, "js")},
        origin_cmd=http_proxy_origin(hyper_hello_exe))


def deno_core_single(exe):
    print "http_benchmark testing deno_core_single"
    return run([exe, "--single-thread"])


def deno_core_multi(exe):
    print "http_benchmark testing deno_core_multi"
    return run([exe, "--multi-thread"])


def node_http():
    node_cmd = ["node", "tools/node_http.js", ADDR.split(":")[1]]
    print "http_benchmark testing NODE."
    return run(node_cmd)


def node_http_proxy(hyper_hello_exe):
    node_cmd = ["node", "tools/node_http_proxy.js", ADDR.split(":")[1]]
    print "http_proxy_benchmark testing NODE."
    return run(node_cmd, None, http_proxy_origin(hyper_hello_exe))


def node_tcp_proxy(hyper_hello_exe):
    node_cmd = [
        "node", "tools/node_tcp_proxy.js",
        ADDR.split(":")[1],
        ORIGIN_ADDR.split(":")[1]
    ]
    print "http_proxy_benchmark testing NODE tcp."
    return run(node_cmd, None, http_proxy_origin(hyper_hello_exe))


def node_tcp():
    node_cmd = ["node", "tools/node_tcp.js", ADDR.split(":")[1]]
    print "http_benchmark testing node_tcp.js"
    return run(node_cmd)


def http_proxy_origin(hyper_hello_exe):
    return [hyper_hello_exe, ORIGIN_ADDR.split(":")[1]]


def hyper_http(hyper_hello_exe):
    hyper_cmd = [hyper_hello_exe, ADDR.split(":")[1]]
    print "http_benchmark testing RUST hyper."
    return run(hyper_cmd)


def http_benchmark(build_dir):
    hyper_hello_exe = os.path.join(build_dir, "hyper_hello")
    core_http_bench_exe = os.path.join(build_dir, "deno_core_http_bench")
    deno_exe = os.path.join(build_dir, "deno")
    return {
        # "deno_tcp" was once called "deno"
        "deno_tcp": deno_tcp(deno_exe),
        # "deno_http" was once called "deno_net_http"
        "deno_http": deno_http(deno_exe),
        "deno_proxy": deno_http_proxy(deno_exe, hyper_hello_exe),
        "deno_proxy_tcp": deno_tcp_proxy(deno_exe, hyper_hello_exe),
        "deno_core_single": deno_core_single(core_http_bench_exe),
        "deno_core_multi": deno_core_multi(core_http_bench_exe),
        # "node_http" was once called "node"
        "node_http": node_http(),
        "node_proxy": node_http_proxy(hyper_hello_exe),
        "node_proxy_tcp": node_tcp_proxy(hyper_hello_exe),
        "node_tcp": node_tcp(),
        "hyper": hyper_http(hyper_hello_exe)
    }


def run(server_cmd, merge_env=None, origin_cmd=None):
    # Run deno echo server in the background.
    if merge_env is None:
        env = None
    else:
        env = os.environ.copy()
        for key, value in merge_env.iteritems():
            env[key] = value

    # Wait for port 4544 to become available.
    # TODO Need to use SO_REUSEPORT with tokio::net::TcpListener.
    time.sleep(5)

    origin = None
    if origin_cmd is not None:
        print "Starting origin server"
        origin = subprocess.Popen(origin_cmd, env=env)

    server = subprocess.Popen(server_cmd, env=env)

    time.sleep(15)  # wait for server to wake up. TODO racy.

    try:
        cmd = "third_party/wrk/%s/wrk -d %s http://%s/" % (util.platform(),
                                                           DURATION, ADDR)
        print cmd
        output = subprocess.check_output(cmd, shell=True)
        stats = util.parse_wrk_output(output)
        print output
        return stats
    finally:
        server.kill()
        if origin is not None:
            print "Stopping origin server"
            origin.kill()


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/http_benchmark.py target/debug/deno"
        sys.exit(1)
    deno_http(sys.argv[1])
