#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import pty
import select
import subprocess
import sys
import time

import http_server
from util import build_path, root_path, executable_suffix, green_ok, red_failed

PERMISSIONS_PROMPT_TEST_TS = "tools/complex_permissions_test.ts"

PROMPT_PATTERN = b'⚠️'
PERMISSION_DENIED_PATTERN = b'PermissionDenied: permission denied'


# This function is copied from:
# https://gist.github.com/hayd/4f46a68fc697ba8888a7b517a414583e
# https://stackoverflow.com/q/52954248/1240268
def tty_capture(cmd, bytes_input, timeout=5):
    """Capture the output of cmd with bytes_input to stdin,
    with stdin, stdout and stderr as TTYs."""
    mo, so = pty.openpty()  # provide tty to enable line-buffering
    me, se = pty.openpty()
    mi, si = pty.openpty()
    fdmap = {mo: 'stdout', me: 'stderr', mi: 'stdin'}

    timeout_exact = time.time() + timeout
    p = subprocess.Popen(
        cmd, bufsize=1, stdin=si, stdout=so, stderr=se, close_fds=True)
    os.write(mi, bytes_input)

    select_timeout = .04  #seconds
    res = {'stdout': b'', 'stderr': b''}
    while True:
        ready, _, _ = select.select([mo, me], [], [], select_timeout)
        if ready:
            for fd in ready:
                data = os.read(fd, 512)
                if not data:
                    break
                res[fdmap[fd]] += data
        elif p.poll() is not None or time.time(
        ) > timeout_exact:  # select timed-out
            break  # p exited
    for fd in [si, so, se, mi, mo, me]:
        os.close(fd)  # can't do it sooner: it leads to errno.EIO error
    p.wait()
    return p.returncode, res['stdout'], res['stderr']


# Wraps a test in debug printouts
# so we have visual indicator of what test failed
def wrap_test(test_name, test_method, *argv):
    sys.stdout.write(test_name + " ... ")
    try:
        test_method(*argv)
        print green_ok()
    except AssertionError:
        print red_failed()
        raise


class Prompt(object):
    def __init__(self, deno_exe, test_types):
        self.deno_exe = deno_exe
        self.test_types = test_types

    def run(self, flags, args, bytes_input):
        "Returns (return_code, stdout, stderr)."
        cmd = [self.deno_exe, "run"] + flags + [PERMISSIONS_PROMPT_TEST_TS
                                                ] + args
        print " ".join(cmd)
        return tty_capture(cmd, bytes_input)

    def warm_up(self):
        # ignore the ts compiling message
        self.run(["--allow-read"], ["read", "package.json"], b'')

    def test(self):
        for test_type in ["read", "write"]:
            test_name_base = "test_" + test_type
            wrap_test(test_name_base + "_inside_project_dir",
                      self.test_inside_project_dir, test_type)
            wrap_test(test_name_base + "_outside_tests_dir",
                      self.test_outside_test_dir, test_type)
            wrap_test(test_name_base + "_inside_tests_dir",
                      self.test_inside_test_dir, test_type)
            wrap_test(test_name_base + "_outside_tests_and_js_dir",
                      self.test_outside_test_and_js_dir, test_type)
            wrap_test(test_name_base + "_inside_tests_and_js_dir",
                      self.test_inside_test_and_js_dir, test_type)
            wrap_test(test_name_base + "_relative", self.test_relative,
                      test_type)
            wrap_test(test_name_base + "_no_prefix", self.test_no_prefix,
                      test_type)

        test_name = "net_fetch"
        test_name_base = "test_" + test_name
        wrap_test(test_name_base + "_allow_localhost_4545",
                  self.test_allow_localhost_4545, test_name,
                  ["http://localhost:4545"])
        wrap_test(test_name_base + "_allow_deno_land",
                  self.test_allow_deno_land, test_name,
                  ["http://localhost:4545"])
        wrap_test(test_name_base + "_allow_localhost_4545_fail",
                  self.test_allow_localhost_4545_fail, test_name,
                  ["http://localhost:4546"])
        wrap_test(test_name_base + "_allow_localhost",
                  self.test_allow_localhost, test_name, [
                      "http://localhost:4545", "http://localhost:4546",
                      "http://localhost:4547"
                  ])

        test_name = "net_dial"
        test_name_base = "test_" + test_name
        wrap_test(test_name_base + "_allow_localhost_4545",
                  self.test_allow_localhost_4545, test_name,
                  ["localhost:4545"])
        wrap_test(test_name_base + "_allow_deno_land",
                  self.test_allow_deno_land, test_name, ["localhost:4545"])
        wrap_test(test_name_base + "_allow_localhost_4545_fail",
                  self.test_allow_localhost_4545_fail, test_name,
                  ["localhost:4546"])
        wrap_test(test_name_base + "_allow_localhost",
                  self.test_allow_localhost, test_name,
                  ["localhost:4545", "localhost:4546", "localhost:4547"])

        test_name = "net_listen"
        test_name_base = "test_" + test_name
        wrap_test(test_name_base + "_allow_localhost_4555",
                  self.test_allow_localhost_4555, test_name,
                  ["localhost:4555"])
        wrap_test(test_name_base + "_allow_deno_land",
                  self.test_allow_deno_land, test_name, ["localhost:4545"])
        wrap_test(test_name_base + "_allow_localhost_4555_fail",
                  self.test_allow_localhost_4555_fail, test_name,
                  ["localhost:4556"])
        wrap_test(test_name_base + "_allow_localhost",
                  self.test_allow_localhost, test_name,
                  ["localhost:4555", "localhost:4556", "localhost:4557"])

    # read/write tests
    def test_inside_project_dir(self, test_type):
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-" + test_type + "=" + root_path],
            [test_type, "package.json", "tests/subdir/config.json"], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_outside_test_dir(self, test_type):
        code, _stdout, stderr = self.run([
            "--no-prompt",
            "--allow-" + test_type + "=" + os.path.join(root_path, "tests")
        ], [test_type, "package.json"], b'')
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_inside_test_dir(self, test_type):
        code, _stdout, stderr = self.run([
            "--no-prompt",
            "--allow-" + test_type + "=" + os.path.join(root_path, "tests")
        ], [test_type, "tests/subdir/config.json"], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_outside_test_and_js_dir(self, test_type):
        code, _stdout, stderr = self.run([
            "--no-prompt", "--allow-" + test_type + "=" + os.path.join(
                root_path, "tests") + "," + os.path.join(root_path, "js")
        ], [test_type, "package.json"], b'')
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_inside_test_and_js_dir(self, test_type):
        code, _stdout, stderr = self.run([
            "--no-prompt", "--allow-" + test_type + "=" + os.path.join(
                root_path, "tests") + "," + os.path.join(root_path, "js")
        ], [test_type, "js/dir_test.ts", "tests/subdir/config.json"], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_relative(self, test_type):
        # Save and restore curdir
        saved_curdir = os.getcwd()
        os.chdir(root_path)
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-" + test_type + "=" + "./tests"],
            [test_type, "tests/subdir/config.json"], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr
        os.chdir(saved_curdir)

    def test_no_prefix(self, test_type):
        # Save and restore curdir
        saved_curdir = os.getcwd()
        os.chdir(root_path)
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-" + test_type + "=" + "tests"],
            [test_type, "tests/subdir/config.json"], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr
        os.chdir(saved_curdir)

    # net tests
    def test_allow_localhost_4545(self, test_type, hosts):
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-net=localhost:4545"], [test_type] + hosts,
            b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost_4555(self, test_type, hosts):
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-net=localhost:4555"], [test_type] + hosts,
            b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_allow_deno_land(self, test_type, hosts):
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-net=deno.land"], [test_type] + hosts, b'')
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost_4545_fail(self, test_type, hosts):
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-net=localhost:4545"], [test_type] + hosts,
            b'')
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost_4555_fail(self, test_type, hosts):
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-net=localhost:4555"], [test_type] + hosts,
            b'')
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost(self, test_type, hosts):
        code, _stdout, stderr = self.run(
            ["--no-prompt", "--allow-net=localhost"], [test_type] + hosts, b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr


def complex_permissions_test(deno_exe):
    p = Prompt(deno_exe, ["read", "write", "net"])
    p.test()


def main():
    print "Permissions prompt tests"
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    http_server.spawn()
    complex_permissions_test(deno_exe)


if __name__ == "__main__":
    main()
