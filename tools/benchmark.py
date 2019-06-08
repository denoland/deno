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
from util import root_path, run, run_output, build_path, executable_suffix
import tempfile
import http_server
import throughput_benchmark
from http_benchmark import http_benchmark
import prebuilt
import subprocess

# The list of the tuples of the benchmark name and arguments
exec_time_benchmarks = [
    ("hello", ["tests/002_hello.ts"]),
    ("relative_import", ["tests/003_relative_import.ts"]),
    ("error_001", ["tests/error_001.ts"]),
    ("cold_hello", ["--reload", "tests/002_hello.ts"]),
    ("cold_relative_import", ["--reload", "tests/003_relative_import.ts"]),
    ("workers_startup", ["tests/workers_startup_bench.ts"]),
    ("workers_round_robin", ["tests/workers_round_robin_bench.ts"]),
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
        "deno":
        os.path.join(build_dir, "deno" + executable_suffix),
        "main.js":
        os.path.join(build_dir, "gen/cli/bundle/main.js"),
        "main.js.map":
        os.path.join(build_dir, "gen/cli/bundle/main.js.map"),
        "compiler.js":
        os.path.join(build_dir, "gen/cli/bundle/compiler.js"),
        "compiler.js.map":
        os.path.join(build_dir, "gen/cli/bundle/compiler.js.map"),
        "snapshot_deno.bin":
        os.path.join(build_dir, "gen/cli/snapshot_deno.bin"),
        "snapshot_compiler.bin":
        os.path.join(build_dir, "gen/cli/snapshot_compiler.bin")
    }
    sizes = {}
    for name, path in path_dict.items():
        assert os.path.exists(path)
        sizes[name] = os.path.getsize(path)
    return sizes


def get_strace_summary_text(test_args):
    f = tempfile.NamedTemporaryFile()
    cmd = ["strace", "-c", "-f", "-o", f.name] + test_args
    try:
        subprocess.check_output(cmd)
    except subprocess.CalledProcessError:
        pass
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


def run_throughput(deno_exe):
    m = {}
    m["100M_tcp"] = throughput_benchmark.tcp(deno_exe, 100)
    m["100M_cat"] = throughput_benchmark.cat(deno_exe, 100)
    m["10M_tcp"] = throughput_benchmark.tcp(deno_exe, 10)
    m["10M_cat"] = throughput_benchmark.cat(deno_exe, 10)
    return m


# "thread_count" and "syscall_count" are both calculated here.
def run_strace_benchmarks(deno_exe, new_data):
    thread_count = {}
    syscall_count = {}
    for (name, args) in exec_time_benchmarks:
        s = get_strace_summary([deno_exe, "run"] + args)
        thread_count[name] = s["clone"]["calls"] + 1
        syscall_count[name] = s["total"]["calls"]
    new_data["thread_count"] = thread_count
    new_data["syscall_count"] = syscall_count


# Takes the output from "/usr/bin/time -v" as input and extracts the 'maximum
# resident set size' and returns it in bytes.
def find_max_mem_in_bytes(time_v_output):
    for line in time_v_output.split('\n'):
        if 'maximum resident set size (kbytes)' in line.lower():
            _, value = line.split(': ')
            return int(value) * 1024


def run_max_mem_benchmark(deno_exe):
    results = {}
    for (name, args) in exec_time_benchmarks:
        cmd = ["/usr/bin/time", "-v", deno_exe, "run"] + args
        try:
            out = subprocess.check_output(cmd, stderr=subprocess.STDOUT)
        except subprocess.CalledProcessError:
            pass
        mem = find_max_mem_in_bytes(out)
        results[name] = mem
    return results


def run_exec_time(deno_exe, build_dir):
    benchmark_file = os.path.join(build_dir, "hyperfine_results.json")
    hyperfine = prebuilt.load_hyperfine()
    run([
        hyperfine, "--ignore-failure", "--export-json", benchmark_file,
        "--warmup", "3"
    ] + [
        deno_exe + " run " + " ".join(args)
        for [_, args] in exec_time_benchmarks
    ])
    hyperfine_results = read_json(benchmark_file)
    results = {}
    for [[name, _], data] in zip(exec_time_benchmarks,
                                 hyperfine_results["results"]):
        results[name] = {
            "mean": data["mean"],
            "stddev": data["stddev"],
            "user": data["user"],
            "system": data["system"],
            "min": data["min"],
            "max": data["max"]
        }
    return results


def run_http(build_dir, new_data):
    stats = http_benchmark(build_dir)
    new_data["req_per_sec"] = {k: v["req_per_sec"] for k, v in stats.items()}
    new_data["max_latency"] = {k: v["max_latency"] for k, v in stats.items()}


def main(argv):
    if len(argv) == 2:
        build_dir = sys.argv[1]
    elif len(argv) == 1:
        build_dir = build_path()
    else:
        print "Usage: tools/benchmark.py [build_dir]"
        sys.exit(1)

    sha1 = run_output(["git", "rev-parse", "HEAD"],
                      exit_on_fail=True).out.strip()
    http_server.spawn()

    deno_exe = os.path.join(build_dir, "deno")

    os.chdir(root_path)
    import_data_from_gh_pages()

    new_data = {
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "sha1": sha1,
    }

    # TODO(ry) The "benchmark" benchmark should actually be called "exec_time".
    # When this is changed, the historical data in gh-pages branch needs to be
    # changed too.
    new_data["benchmark"] = run_exec_time(deno_exe, build_dir)

    new_data["binary_size"] = get_binary_sizes(build_dir)

    # Cannot run throughput benchmark on windows because they don't have nc or
    # pipe.
    if os.name != 'nt':
        new_data["throughput"] = run_throughput(deno_exe)
        run_http(build_dir, new_data)

    if "linux" in sys.platform:
        run_strace_benchmarks(deno_exe, new_data)
        new_data["max_memory"] = run_max_mem_benchmark(deno_exe)

    print "===== <BENCHMARK RESULTS>"
    print json.dumps(new_data, indent=2)
    print "===== </BENCHMARK RESULTS>"

    all_data = read_json(all_data_file)
    all_data.append(new_data)

    write_json(all_data_file, all_data)
    write_json(recent_data_file, all_data[-20:])


if __name__ == '__main__':
    main(sys.argv)
