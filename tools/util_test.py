# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os

from test_util import DenoTestCase, run_tests
from util import (parse_exit_code, shell_quote_win, parse_wrk_output,
                  root_path)


class TestUtil(DenoTestCase):
    def test_parse_exit_code(self):
        assert parse_exit_code('hello_error54_world') == 54
        assert parse_exit_code('hello_error_world') == 1
        assert parse_exit_code('hello_world') == 0

    def test_shell_quote_win(self):
        assert shell_quote_win('simple') == 'simple'
        assert shell_quote_win(
            'roof/\\isoprojection') == 'roof/\\isoprojection'
        assert shell_quote_win('with space') == '"with space"'
        assert shell_quote_win('embedded"quote') == '"embedded""quote"'
        assert shell_quote_win(
            'a"b""c\\d\\"e\\\\') == '"a""b""""c\\d\\\\""e\\\\\\\\"'

    def test_parse_wrk_output(self):
        f = open(os.path.join(root_path, "tools/testdata/wrk1.txt"))
        stats = parse_wrk_output(f.read())
        assert stats['req_per_sec'] == 1837
        assert stats['max_latency'] == 6.25

        f2 = open(os.path.join(root_path, "tools/testdata/wrk2.txt"))
        stats2 = parse_wrk_output(f2.read())
        assert stats2['req_per_sec'] == 53435
        assert stats2['max_latency'] == 6.22

        f3 = open(os.path.join(root_path, "tools/testdata/wrk3.txt"))
        stats3 = parse_wrk_output(f3.read())
        assert stats3['req_per_sec'] == 96037
        assert stats3['max_latency'] == 6.36

    def test_executable_exists(self):
        assert os.path.exists(self.deno_exe)


if __name__ == '__main__':
    run_tests()
