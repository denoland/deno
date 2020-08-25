#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
import sys
import time
import subprocess
import util
import third_party

# Some of the benchmarks in this file have been renamed. In case the history
# somehow gets messed up:
#   "node_http" was once called "node"
#   "deno_tcp" was once called "deno"
#   "deno_http" was once called "deno_net_http"

DURATION = "20s"
NEXT_PORT = 4544


def server_addr(port):
    return "0.0.0.0:%s" % port


def get_port():
    global NEXT_PORT
    port = NEXT_PORT
    NEXT_PORT += 1
    # Return port as str because all usages below are as a str and having it an
    # integer just adds complexity.
    return str(port)


def deno_tcp(deno_exe):
    port = get_port()
    deno_cmd = [
        # TODO(lucacasonato): remove unstable when stabilized
        deno_exe,
        "run",
        "--allow-net",
        "tools/deno_tcp.ts",
        server_addr(port)
    ]
    print "http_benchmark testing DENO tcp."
    return run(deno_cmd, port)


def deno_http(deno_exe):
    port = get_port()
    deno_cmd = [
        deno_exe, "run", "--allow-net", "--reload", "--unstable",
        "std/http/http_bench.ts",
        server_addr(port)
    ]
    print "http_benchmark testing DENO using net/http."
    return run(deno_cmd, port)


def deno_tcp_proxy(deno_exe, hyper_hello_exe):
    port = get_port()
    origin_port = get_port()
    deno_cmd = [
        deno_exe, "run", "--allow-net", "tools/deno_tcp_proxy.ts",
        server_addr(port),
        server_addr(origin_port)
    ]
    print "http_proxy_benchmark testing DENO using net/tcp."
    return run(
        deno_cmd,
        port,
        origin_cmd=http_proxy_origin(hyper_hello_exe, origin_port))


def deno_http_proxy(deno_exe, hyper_hello_exe):
    port = get_port()
    origin_port = get_port()
    deno_cmd = [
        deno_exe, "run", "--allow-net", "tools/deno_http_proxy.ts",
        server_addr(port),
        server_addr(origin_port)
    ]
    print "http_proxy_benchmark testing DENO using net/http."
    return run(
        deno_cmd,
        port,
        origin_cmd=http_proxy_origin(hyper_hello_exe, origin_port))


def core_http_bin_ops(exe):
    print "http_benchmark testing CORE http_bench_bin_ops"
    return run([exe], 4544)


def core_http_json_ops(exe):
    print "http_benchmark testing CORE http_bench_json_ops"
    return run([exe], 4544)


def node_http():
    port = get_port()
    node_cmd = ["node", "tools/node_http.js", port]
    print "http_benchmark testing NODE."
    return run(node_cmd, port)


def node_http_proxy(hyper_hello_exe):
    port = get_port()
    origin_port = get_port()
    node_cmd = ["node", "tools/node_http_proxy.js", port, origin_port]
    print "http_proxy_benchmark testing NODE."
    return run(node_cmd, port, None,
               http_proxy_origin(hyper_hello_exe, origin_port))


def node_tcp_proxy(hyper_hello_exe):
    port = get_port()
    origin_port = get_port()
    node_cmd = ["node", "tools/node_tcp_proxy.js", port, origin_port]
    print "http_proxy_benchmark testing NODE tcp."
    return run(node_cmd, port, None,
               http_proxy_origin(hyper_hello_exe, origin_port))


def node_tcp():
    port = get_port()
    node_cmd = ["node", "tools/node_tcp.js", port]
    print "http_benchmark testing node_tcp.js"
    return run(node_cmd, port)


def http_proxy_origin(hyper_hello_exe, port):
    return [hyper_hello_exe, port]


def hyper_http(hyper_hello_exe):
    port = get_port()
    hyper_cmd = [hyper_hello_exe, port]
    print "http_benchmark testing RUST hyper."
    return run(hyper_cmd, port)


def http_benchmark(build_dir):
    deno_exe = os.path.join(build_dir, "deno")
    hyper_hello_exe = os.path.join(build_dir, "test_server")
    core_http_bin_ops_exe = os.path.join(build_dir,
                                         "examples/http_bench_bin_ops")
    core_http_json_ops_exe = os.path.join(build_dir,
                                          "examples/http_bench_json_ops")
    return {
        # "deno_tcp" was once called "deno"
        "deno_tcp": deno_tcp(deno_exe),
        # "deno_udp": deno_udp(deno_exe),
        "deno_http": deno_http(deno_exe),
        # TODO(ry) deno_proxy disabled to make fetch() standards compliant.
        # "deno_proxy": deno_http_proxy(deno_exe, hyper_hello_exe),
        "deno_proxy_tcp": deno_tcp_proxy(deno_exe, hyper_hello_exe),
        # "core_http_bin_ops" was once called "deno_core_single"
        # "core_http_bin_ops" was once called "deno_core_http_bench"
        "core_http_bin_ops": core_http_bin_ops(core_http_bin_ops_exe),
        "core_http_json_ops": core_http_json_ops(core_http_json_ops_exe),
        # "node_http" was once called "node"
        "node_http": node_http(),
        "node_proxy": node_http_proxy(hyper_hello_exe),
        "node_proxy_tcp": node_tcp_proxy(hyper_hello_exe),
        "node_tcp": node_tcp(),
        "hyper": hyper_http(hyper_hello_exe)
    }


def run(server_cmd, port, merge_env=None, origin_cmd=None):

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

    print server_cmd
    server = subprocess.Popen(server_cmd, env=env)

    time.sleep(5)  # wait for server to wake up. TODO racy.

    try:
        wrk = third_party.get_prebuilt_tool_path("wrk")
        assert os.path.exists(wrk)
        cmd = "%s -d %s --latency http://127.0.0.1:%s/" % (wrk, DURATION, port)
        print cmd
        output = subprocess.check_output(cmd, shell=True)
        stats = util.parse_wrk_output(output)
        print output
        return stats
    finally:
        server_retcode = server.poll()
        if server_retcode is not None and server_retcode != 0:
            print "server ended with error"
            sys.exit(1)
        server.kill()
        if origin is not None:
            origin.kill()


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print "Usage ./tools/http_benchmark.py target/debug/deno"
        sys.exit(1)
    deno_http(sys.argv[1])
