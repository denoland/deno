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

DURATION = "10s"

LAST_PORT = 4544


def get_addr(port=None):
    global LAST_PORT
    if port is None:
        port = LAST_PORT
        LAST_PORT = LAST_PORT + 1
    return ("127.0.0.1:%d" % (port))


def deno_tcp(deno_exe):
    addr = get_addr()
    deno_cmd = [deno_exe, "run", "--allow-net", "tools/deno_tcp.ts", addr]
    print "http_benchmark testing DENO."
    return run(deno_cmd, addr)


def deno_http(deno_exe):
    addr = get_addr()
    deno_cmd = [
        deno_exe, "run", "--allow-net",
        "js/deps/https/deno.land/std/http/http_bench.ts", addr
    ]
    print "http_benchmark testing DENO using net/http."
    return run(
        deno_cmd,
        addr,
        merge_env={
            # Load from //js/deps/https/deno.land/net/ submodule.
            "DENO_DIR": os.path.join(util.root_path, "js")
        })


def deno_tcp_proxy(deno_exe, hyper_hello_exe):
    addr = get_addr()
    origin_addr = get_addr()
    deno_cmd = [
        deno_exe, "run", "--allow-net", "tools/deno_tcp_proxy.ts", addr,
        origin_addr
    ]
    print "http_proxy_benchmark testing DENO using net/tcp."
    return run(
        deno_cmd,
        addr,
        merge_env={"DENO_DIR": os.path.join(util.root_path, "js")},
        origin_cmd=http_proxy_origin(hyper_hello_exe, origin_addr))


def deno_http_proxy(deno_exe, hyper_hello_exe):
    addr = get_addr()
    origin_addr = get_addr()
    deno_cmd = [
        deno_exe, "run", "--allow-net", "tools/deno_http_proxy.ts", addr,
        origin_addr
    ]
    print "http_proxy_benchmark testing DENO using net/http."
    return run(
        deno_cmd,
        addr,
        merge_env={"DENO_DIR": os.path.join(util.root_path, "js")},
        origin_cmd=http_proxy_origin(hyper_hello_exe, origin_addr))


def deno_core_single(exe):
    print "http_benchmark testing deno_core_single"
    return run([exe, "--single-thread"], "127.0.0.1:4544")


def deno_core_multi(exe):
    print "http_benchmark testing deno_core_multi"
    return run([exe, "--multi-thread"], "127.0.0.1:4544")


def node_http():
    addr = get_addr()
    node_cmd = ["node", "tools/node_http.js", addr.split(":")[1]]
    print "http_benchmark testing NODE."
    return run(node_cmd, addr)


def node_http_proxy(hyper_hello_exe):
    addr = get_addr()
    origin_addr = get_addr()
    node_cmd = [
        "node", "tools/node_http_proxy.js",
        addr.split(":")[1],
        origin_addr.split(":")[1]
    ]
    print "http_proxy_benchmark testing NODE."
    return run(node_cmd, addr, None,
               http_proxy_origin(hyper_hello_exe, origin_addr))


def node_tcp_proxy(hyper_hello_exe):
    addr = get_addr()
    origin_addr = get_addr()
    node_cmd = [
        "node", "tools/node_tcp_proxy.js",
        addr.split(":")[1],
        origin_addr.split(":")[1]
    ]
    print "http_proxy_benchmark testing NODE tcp."
    return run(node_cmd, addr, None,
               http_proxy_origin(hyper_hello_exe, origin_addr))


def node_tcp():
    addr = get_addr()
    node_cmd = ["node", "tools/node_tcp.js", addr.split(":")[1]]
    print "http_benchmark testing node_tcp.js"
    return run(node_cmd, addr)


def http_proxy_origin(hyper_hello_exe, addr):
    return [hyper_hello_exe, addr.split(":")[1]]


def hyper_http(hyper_hello_exe):
    addr = get_addr()
    hyper_cmd = [hyper_hello_exe, addr.split(":")[1]]
    print "http_benchmark testing RUST hyper."
    return run(hyper_cmd, addr)


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


def run(server_cmd, addr, merge_env=None, origin_cmd=None):

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
        origin = subprocess.Popen(origin_cmd, env=env)

    server = subprocess.Popen(server_cmd, env=env)

    time.sleep(10)  # wait for server to wake up. TODO racy.

    try:
        cmd = "third_party/wrk/%s/wrk -d %s http://%s/" % (util.platform(),
                                                           DURATION, addr)
        print cmd
        output = subprocess.check_output(cmd, shell=True)
        stats = util.parse_wrk_output(output)
        print output
        return stats
    finally:
        server.kill()
        if origin is not None:
            origin.kill()


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/http_benchmark.py target/debug/deno"
        sys.exit(1)
    deno_http(sys.argv[1])
