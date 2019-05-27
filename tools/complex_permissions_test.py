#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import pty
import select
import subprocess
import sys
import time
import unittest

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


class ComplexPermissionTestCase(unittest.TestCase):
    def __init__(self, method_name, test_type, deno_exe):
        super(ComplexPermissionTestCase, self).__init__(method_name)
        self.test_type = test_type
        self.deno_exe = deno_exe

    def _run_deno(self, flags, args):
        "Returns (return_code, stdout, stderr)."
        cmd = ([self.deno_exe, "run", "--no-prompt"] + flags +
               [PERMISSIONS_PROMPT_TEST_TS] + args)
        return tty_capture(cmd, b'')


class TestReadWritePermissions(ComplexPermissionTestCase):
    def test_inside_project_dir(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-" + self.test_type + "=" + root_path],
            [self.test_type, "package.json", "tests/subdir/config.json"])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_outside_test_dir(self):
        code, _stdout, stderr = self._run_deno([
            "--allow-" + self.test_type + "=" + os.path.join(
                root_path, "tests")
        ], [self.test_type, "package.json"])
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_inside_test_dir(self):
        code, _stdout, stderr = self._run_deno([
            "--allow-" + self.test_type + "=" + os.path.join(
                root_path, "tests")
        ], [self.test_type, "tests/subdir/config.json"])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_outside_test_and_js_dir(self):
        code, _stdout, stderr = self._run_deno([
            "--allow-" + self.test_type + "=" + os.path.join(
                root_path, "tests") + "," + os.path.join(root_path, "js")
        ], [self.test_type, "package.json"])
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_inside_test_and_js_dir(self):
        code, _stdout, stderr = self._run_deno([
            "--allow-" + self.test_type + "=" + os.path.join(
                root_path, "tests") + "," + os.path.join(root_path, "js")
        ], [self.test_type, "js/dir_test.ts", "tests/subdir/config.json"])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_relative(self):
        # Save and restore curdir
        saved_curdir = os.getcwd()
        os.chdir(root_path)
        code, _stdout, stderr = self._run_deno(
            ["--allow-" + self.test_type + "=" + "./tests"],
            [self.test_type, "tests/subdir/config.json"])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr
        os.chdir(saved_curdir)

    def test_no_prefix(self):
        # Save and restore curdir
        saved_curdir = os.getcwd()
        os.chdir(root_path)
        code, _stdout, stderr = self._run_deno(
            ["--allow-" + self.test_type + "=" + "tests"],
            [self.test_type, "tests/subdir/config.json"])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr
        os.chdir(saved_curdir)


class TestNetFetchPermissions(ComplexPermissionTestCase):
    def test_allow_localhost_4545(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=localhost:4545"],
            [self.test_type, "http://localhost:4545"])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_allow_deno_land(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=deno.land"],
            [self.test_type, "http://localhost:4545"])
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost_4545_fail(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=localhost:4545"],
            [self.test_type, "http://localhost:4546"])
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost(self):
        code, _stdout, stderr = self._run_deno(["--allow-net=localhost"], [
            self.test_type, "http://localhost:4545", "http://localhost:4546",
            "http://localhost:4547"
        ])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr


class TestNetDialPermissions(ComplexPermissionTestCase):
    def test_allow_localhost_ip_4555(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=127.0.0.1:4545"], [self.test_type, "127.0.0.1:4545"])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_allow_deno_land(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=deno.land"], [self.test_type, "127.0.0.1:4545"])
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost_ip_4545_fail(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=127.0.0.1:4545"], [self.test_type, "127.0.0.1:4546"])
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost_ip(self):
        code, _stdout, stderr = self._run_deno(["--allow-net=127.0.0.1"], [
            self.test_type, "127.0.0.1:4545", "127.0.0.1:4546",
            "127.0.0.1:4547"
        ])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr


class TestNetListenPermissions(ComplexPermissionTestCase):
    def test_allow_localhost_4555(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=localhost:4555"], [self.test_type, "localhost:4555"])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_allow_deno_land(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=deno.land"], [self.test_type, "localhost:4545"])
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost_4555_fail(self):
        code, _stdout, stderr = self._run_deno(
            ["--allow-net=localhost:4555"], [self.test_type, "localhost:4556"])
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_allow_localhost(self):
        code, _stdout, stderr = self._run_deno(["--allow-net=localhost"], [
            self.test_type, "localhost:4555", "localhost:4556",
            "localhost:4557"
        ])
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not PERMISSION_DENIED_PATTERN in stderr


def complex_permissions_test(deno_exe):
    runner = unittest.TextTestRunner(verbosity=2)
    loader = unittest.TestLoader()

    tests = (
        ("read", TestReadWritePermissions),
        ("write", TestReadWritePermissions),
        ("net_fetch", TestNetFetchPermissions),
        ("net_dial", TestNetDialPermissions),
        ("net_listen", TestNetListenPermissions),
    )

    for (test_type, test_class) in tests:
        print "Complex permissions tests for \"{}\"".format(test_type)

        test_names = loader.getTestCaseNames(test_class)
        suite = unittest.TestSuite()
        for test_name in test_names:
            suite.addTest(test_class(test_name, test_type, deno_exe))

        result = runner.run(suite)
        if not result.wasSuccessful():
            sys.exit(1)


def main():
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    http_server.spawn()
    complex_permissions_test(deno_exe)


if __name__ == "__main__":
    main()
