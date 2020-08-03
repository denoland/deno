#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# Performs benchmark and append data to //website/data.json.
# If //website/data.json doesn't exist, this script tries to import it from
# gh-pages branch.
# To view the results locally run target/debug/test_server and visit
# http://localhost:4545/website

import os
import sys
import json
import time
import tempfile
import subprocess
from util import build_path, executable_suffix, root_path, run, run_output
import third_party
from http_benchmark import http_benchmark
import throughput_benchmark

# The list of the tuples of the benchmark name, arguments and return code
exec_time_benchmarks = [
    ("hello", ["run", "cli/tests/002_hello.ts"], None),
    ("relative_import", ["run", "cli/tests/003_relative_import.ts"], None),
    ("error_001", ["run", "cli/tests/error_001.ts"], 1),
    ("cold_hello", ["run", "--reload", "cli/tests/002_hello.ts"], None),
    ("cold_relative_import",
     ["run", "--reload", "cli/tests/003_relative_import.ts"], None),
    ("workers_startup",
     ["run", "--allow-read", "cli/tests/workers_startup_bench.ts"], None),
    ("workers_round_robin",
     ["run", "--allow-read", "cli/tests/workers_round_robin_bench.ts"], None),
    ("text_decoder", ["run", "cli/tests/text_decoder_perf.js"], None),
    ("text_encoder", ["run", "cli/tests/text_encoder_perf.js"], None),
    ("check", ["cache", "--reload", "std/examples/chat/server_test.ts"], None),
    ("no_check",
     ["cache", "--reload", "--no-check",
      "std/examples/chat/server_test.ts"], None),
]


def read_json(filename):
    with open(filename) as json_file:
        return json.load(json_file)


def write_json(filename, data):
    with open(filename, 'w') as outfile:
        json.dump(data, outfile)


def get_binary_sizes(build_dir):
    sizes = {}
    mtimes = {}
    # The deno executable should be located at the root of the build tree.
    deno_exe = os.path.join(build_dir, "deno" + executable_suffix)
    sizes["deno"] = os.path.getsize(deno_exe)
    # Because cargo's OUT_DIR is not predictable, search the build tree for
    # snapshot related files.
    for parent_dir, _, file_names in os.walk(build_dir):
        for file_name in file_names:
            if not file_name in [
                    "CLI_SNAPSHOT.bin",
                    "COMPILER_SNAPSHOT.bin",
            ]:
                continue
            file_path = os.path.join(parent_dir, file_name)
            file_mtime = os.path.getmtime(file_path)
            # If multiple copies of a file are found, use the most recent one.
            if file_name in mtimes and mtimes[file_name] > file_mtime:
                continue
            mtimes[file_name] = file_mtime
            sizes[file_name] = os.path.getsize(file_path)
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
    # Filter out non-relevant lines. See the error log at
    # https://github.com/denoland/deno/pull/3715/checks?check_run_id=397365887
    # This is checked in tools/testdata/strace_summary2.out
    lines = [x for x in lines if x.find("detached ...") == -1]
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
    s = get_strace_summary_text(test_args)
    try:
        return strace_parse(s)
    except ValueError:
        print "error parsing strace"
        print "----- <strace> -------"
        print s
        print "----- </strace> ------"


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
    for (name, args, _) in exec_time_benchmarks:
        s = get_strace_summary([deno_exe] + args)
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
    for (name, args, return_code) in exec_time_benchmarks:
        cmd = ["/usr/bin/time", "-v", deno_exe] + args
        try:
            out = subprocess.check_output(cmd, stderr=subprocess.STDOUT)
        except subprocess.CalledProcessError as e:
            if (return_code is e.returncode):
                pass
            else:
                raise e
        mem = find_max_mem_in_bytes(out)
        results[name] = mem
    return results


def run_exec_time(deno_exe, build_dir):
    hyperfine_exe = third_party.get_prebuilt_tool_path("hyperfine")
    benchmark_file = os.path.join(build_dir, "hyperfine_results.json")

    def benchmark_command(deno_exe, args, return_code):
        # Bash test which asserts the return code value of the previous command
        # $? contains the return code of the previous command
        return_code_test = "; test $? -eq {}".format(
            return_code) if return_code is not None else ""
        return "{} {}{}".format(deno_exe, " ".join(args), return_code_test)

    run([hyperfine_exe, "--export-json", benchmark_file, "--warmup", "3"] + [
        benchmark_command(deno_exe, args, return_code)
        for (_, args, return_code) in exec_time_benchmarks
    ])
    hyperfine_results = read_json(benchmark_file)
    results = {}
    for [[name, _, _], data] in zip(exec_time_benchmarks,
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


def bundle_benchmark(deno_exe):
    bundles = {
        "file_server": "./std/http/file_server.ts",
        "gist": "./std/examples/gist.ts",
    }

    sizes = {}

    for name, url in bundles.items():
        # bundle
        path = name + ".bundle.js"
        run([deno_exe, "bundle", "--unstable", url, path])
        # get size of bundle
        assert os.path.exists(path)
        sizes[name] = os.path.getsize(path)
        # remove bundle
        os.remove(path)

    return sizes


def main():
    build_dir = build_path()
    sha1 = run_output(["git", "rev-parse", "HEAD"],
                      exit_on_fail=True).out.strip()

    deno_exe = os.path.join(build_dir, "deno")

    os.chdir(root_path)

    new_data = {
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "sha1": sha1,
    }

    # TODO(ry) The "benchmark" benchmark should actually be called "exec_time".
    # When this is changed, the historical data in gh-pages branch needs to be
    # changed too.
    new_data["benchmark"] = run_exec_time(deno_exe, build_dir)

    new_data["binary_size"] = get_binary_sizes(build_dir)
    new_data["bundle_size"] = bundle_benchmark(deno_exe)

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

    write_json(os.path.join(build_dir, "bench.json"), new_data)


if __name__ == '__main__':
    main()
