#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Performs benchmark and append data to //website/data.json.
# If //website/data.json doesn't exist, this script tries to import it from
# gh-pages branch.
# To view the results locally run ./tools/http_server.py and visit
# http://localhost:4545/website

import os
import sys
import json
import time
import shutil
from util import run, run_output, root_path, build_path, executable_suffix
import tempfile
import http_server
import throughput_benchmark
from http_benchmark import http_benchmark
import prebuilt

# The list of the tuples of the benchmark name and arguments
exec_time_benchmarks = [
    ("hello", ["tests/002_hello.ts"]),
    ("relative_import", ["tests/003_relative_import.ts"]),
    ("error_001", ["tests/error_001.ts"]),
    ("cold_hello", ["tests/002_hello.ts", "--recompile"]),
    ("cold_relative_import", ["tests/003_relative_import.ts", "--recompile"]),
]

gh_pages_data_file = "gh-pages/data.json"
all_data_file = "website/data.json"  # Includes all benchmark data.
recent_data_file = "website/recent.json"  # Includes recent 20 benchmark data.


def read_json(filename):
    with open(filename) as json_file:
        return json.load(json_file)


def write_json(filename, data):
    with open(filename, 'w') as outfile:
        json.dump(data, outfile)


def import_data_from_gh_pages():
    if os.path.exists(all_data_file):
        return
    try:
        run([
            "git", "clone", "--depth", "1", "-b", "gh-pages",
            "https://github.com/denoland/deno.git", "gh-pages"
        ])
        shutil.copy(gh_pages_data_file, all_data_file)
    except ValueError:
        write_json(all_data_file, [])  # writes empty json data


def get_binary_sizes(build_dir):
    path_dict = {
        "deno": os.path.join(build_dir, "deno" + executable_suffix),
        "main.js": os.path.join(build_dir, "gen/bundle/main.js"),
        "main.js.map": os.path.join(build_dir, "gen/bundle/main.js.map"),
        "snapshot_deno.bin": os.path.join(build_dir, "gen/snapshot_deno.bin")
    }
    sizes = {}
    for name, path in path_dict.items():
        sizes[name] = os.path.getsize(path)
    return sizes


def get_strace_summary_text(test_args):
    f = tempfile.NamedTemporaryFile()
    run(["strace", "-c", "-f", "-o", f.name] + test_args)
    return f.read()


def strace_parse(summary_text):
    summary = {}
    # clear empty lines
    lines = list(filter(lambda x: x and x != "\n", summary_text.split("\n")))
    if len(lines) < 4:
        return {}  # malformed summary
    lines, total_line = lines[2:-2], lines[-1]
    # data to dict for each line
    for line in lines:
        syscall_fields = line.split()
        syscall_name = syscall_fields[-1]
        syscall_dict = {}
        if 5 <= len(syscall_fields) <= 6:
            syscall_dict = {
                "% time": float(syscall_fields[0]),
                "seconds": float(syscall_fields[1]),
                "usecs/call": int(syscall_fields[2]),
                "calls": int(syscall_fields[3])
            }
            syscall_dict["errors"] = 0 if len(syscall_fields) < 6 else int(
                syscall_fields[4])
        summary[syscall_name] = syscall_dict
    # record overall (total) data
    total_fields = total_line.split()
    summary["total"] = {
        "% time": float(total_fields[0]),
        "seconds": float(total_fields[1]),
        "calls": int(total_fields[2]),
        "errors": int(total_fields[3])
    }
    return summary


def get_strace_summary(test_args):
    return strace_parse(get_strace_summary_text(test_args))


def run_thread_count_benchmark(deno_path):
    thread_count_map = {}
    thread_count_map["set_timeout"] = get_strace_summary([
        deno_path, "tests/004_set_timeout.ts", "--reload"
    ])["clone"]["calls"] + 1
    thread_count_map["fetch_deps"] = get_strace_summary([
        deno_path, "tests/fetch_deps.ts", "--reload", "--allow-net"
    ])["clone"]["calls"] + 1
    return thread_count_map


def run_throughput(deno_exe):
    m = {}
    m["100M_tcp"] = throughput_benchmark.tcp(deno_exe, 100)
    m["100M_cat"] = throughput_benchmark.cat(deno_exe, 100)
    m["10M_tcp"] = throughput_benchmark.tcp(deno_exe, 10)
    m["10M_cat"] = throughput_benchmark.cat(deno_exe, 10)
    return m


def run_syscall_count_benchmark(deno_path):
    syscall_count_map = {}
    syscall_count_map["hello"] = get_strace_summary(
        [deno_path, "tests/002_hello.ts", "--reload"])["total"]["calls"]
    syscall_count_map["fetch_deps"] = get_strace_summary(
        [deno_path, "tests/fetch_deps.ts", "--reload",
         "--allow-net"])["total"]["calls"]
    return syscall_count_map


def main(argv):
    if len(argv) == 2:
        build_dir = sys.argv[1]
    elif len(argv) == 1:
        build_dir = build_path()
    else:
        print "Usage: tools/benchmark.py [build_dir]"
        sys.exit(1)

    http_server.spawn()

    deno_path = os.path.join(build_dir, "deno")
    benchmark_file = os.path.join(build_dir, "benchmark.json")

    os.chdir(root_path)
    import_data_from_gh_pages()

    prebuilt.load_hyperfine()

    run([
        "hyperfine", "--ignore-failure", "--export-json", benchmark_file,
        "--warmup", "3"
    ] + [
        deno_path + " " + " ".join(args) for [_, args] in exec_time_benchmarks
    ])
    all_data = read_json(all_data_file)
    benchmark_data = read_json(benchmark_file)
    sha1 = run_output(["git", "rev-parse", "HEAD"]).strip()
    new_data = {
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "sha1": sha1,
        "binary_size": {},
        "thread_count": {},
        "syscall_count": {},
        "benchmark": {}
    }
    for [[name, _], data] in zip(exec_time_benchmarks,
                                 benchmark_data["results"]):
        new_data["benchmark"][name] = {
            "mean": data["mean"],
            "stddev": data["stddev"],
            "user": data["user"],
            "system": data["system"],
            "min": data["min"],
            "max": data["max"]
        }

    new_data["binary_size"] = get_binary_sizes(build_dir)
    # Cannot run throughput benchmark on windows because they don't have nc or
    # pipe.
    if os.name != 'nt':
        hyper_hello_path = os.path.join(build_dir, "hyper_hello")
        new_data["throughput"] = run_throughput(deno_path)
        new_data["req_per_sec"] = http_benchmark(deno_path, hyper_hello_path)
    if "linux" in sys.platform:
        # Thread count test, only on linux
        new_data["thread_count"] = run_thread_count_benchmark(deno_path)
        new_data["syscall_count"] = run_syscall_count_benchmark(deno_path)

    all_data.append(new_data)
    write_json(all_data_file, all_data)
    write_json(recent_data_file, all_data[-20:])


if __name__ == '__main__':
    main(sys.argv)
