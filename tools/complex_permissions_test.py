#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import unittest

import http_server
from test_util import DenoTestCase, run_tests
from util import root_path, tty_capture

PERMISSIONS_PROMPT_TEST_TS = "tools/complex_permissions_test.ts"

PROMPT_PATTERN = b'⚠️'
PERMISSION_DENIED_PATTERN = b'PermissionDenied: permission denied'


@unittest.skipIf(os.name == 'nt', "Unable to test tty on Windows")
class BaseComplexPermissionTest(DenoTestCase):
    def _run_deno(self, flags, args):
        "Returns (return_code, stdout, stderr)."
        cmd = ([self.deno_exe, "run", "--no-prompt"] + flags +
               [PERMISSIONS_PROMPT_TEST_TS] + args)
        return tty_capture(cmd, b'')


class BaseReadWritePermissionsTest(object):
    test_type = None

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


class TestReadPermissions(BaseReadWritePermissionsTest,
                          BaseComplexPermissionTest):
    test_type = "read"


class TestWritePermissions(BaseReadWritePermissionsTest,
                           BaseComplexPermissionTest):
    test_type = "write"


class TestNetFetchPermissions(BaseComplexPermissionTest):
    test_type = "net_fetch"

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


class TestNetDialPermissions(BaseComplexPermissionTest):
    test_type = "net_dial"

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


class TestNetListenPermissions(BaseComplexPermissionTest):
    test_type = "net_listen"

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


def complex_permissions_tests():
    return BaseComplexPermissionTest.__subclasses__()


if __name__ == "__main__":
    with http_server.spawn():
        run_tests()
