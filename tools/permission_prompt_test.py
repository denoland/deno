#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import unittest

from test_util import DenoTestCase, run_tests
from util import tty_capture

PERMISSIONS_PROMPT_TEST_TS = "tools/permission_prompt_test.ts"

PROMPT_PATTERN = b'⚠️'
FIRST_CHECK_FAILED_PATTERN = b'First check failed'
PERMISSION_DENIED_PATTERN = b'PermissionDenied: permission denied'


@unittest.skipIf(os.name == 'nt', "Unable to test tty on Windows")
class BasePromptTest(object):
    def _run_deno(self, flags, args, bytes_input):
        "Returns (return_code, stdout, stderr)."
        cmd = [self.deno_exe, "run"] + flags + [PERMISSIONS_PROMPT_TEST_TS
                                                ] + args
        return tty_capture(cmd, bytes_input)

    def test_allow_flag(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            ["--allow-" + test_type], ["needs" + test_type.capitalize()], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_yes_yes(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            [], ["needs" + test_type.capitalize()], b'y\ny\n')
        assert code == 0
        assert PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_yes_no(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            [], ["needs" + test_type.capitalize()], b'y\nn\n')
        assert code == 1
        assert PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_no_no(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            [], ["needs" + test_type.capitalize()], b'n\nn\n')
        assert code == 1
        assert PROMPT_PATTERN in stderr
        assert FIRST_CHECK_FAILED_PATTERN in stdout
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_no_yes(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            [], ["needs" + test_type.capitalize()], b'n\ny\n')
        assert code == 0

        assert PROMPT_PATTERN in stderr
        assert FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_allow(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            [], ["needs" + test_type.capitalize()], b'a\n')
        assert code == 0
        assert PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_deny(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            [], ["needs" + test_type.capitalize()], b'd\n')
        assert code == 1
        assert PROMPT_PATTERN in stderr
        assert FIRST_CHECK_FAILED_PATTERN in stdout
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_unrecognized_option(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            [], ["needs" + test_type.capitalize()], b'e\na\n')
        assert code == 0
        assert PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr
        assert b'Unrecognized option' in stderr

    def test_no_prompt(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            ["--no-prompt"], ["needs" + test_type.capitalize()], b'')
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert FIRST_CHECK_FAILED_PATTERN in stdout
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_no_prompt_allow(self):
        test_type = self.test_type
        code, stdout, stderr = self._run_deno(
            ["--no-prompt", "--allow-" + test_type],
            ["needs" + test_type.capitalize()], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr


class ReadPromptTest(DenoTestCase, BasePromptTest):
    test_type = "read"


class WritePromptTest(DenoTestCase, BasePromptTest):
    test_type = "write"


class EnvPromptTest(DenoTestCase, BasePromptTest):
    test_type = "env"


class NetPromptTest(DenoTestCase, BasePromptTest):
    test_type = "net"


class RunPromptTest(DenoTestCase, BasePromptTest):
    test_type = "run"


def permission_prompt_tests():
    return BasePromptTest.__subclasses__()


if __name__ == "__main__":
    run_tests()
