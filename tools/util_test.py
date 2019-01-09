# Copyright 2018 the Deno authors. All rights reserved. MIT license.
from util import pattern_match, parse_exit_code, shell_quote_win
import util
import os
import sys


def pattern_match_test():
    print "Testing util.pattern_match()..."
    # yapf: disable
    fixtures = [("foobarbaz", "foobarbaz", True),
                ("[WILDCARD]", "foobarbaz", True),
                ("foobar", "foobarbaz", False),
                ("foo[WILDCARD]baz", "foobarbaz", True),
                ("foo[WILDCARD]baz", "foobazbar", False),
                ("foo[WILDCARD]baz[WILDCARD]qux", "foobarbazqatqux", True),
                ("foo[WILDCARD]", "foobar", True),
                ("foo[WILDCARD]baz[WILDCARD]", "foobarbazqat", True)]
    # yapf: enable

    # Iterate through the fixture lists, testing each one
    for (pattern, string, expected) in fixtures:
        actual = pattern_match(pattern, string)
        assert expected == actual, \
            "expected %s for\nExpected:\n%s\nTo equal actual:\n%s" % (
            expected, pattern, string)

    assert pattern_match("foo[BAR]baz", "foobarbaz",
                         "[BAR]") == True, "expected wildcard to be set"
    assert pattern_match("foo[BAR]baz", "foobazbar",
                         "[BAR]") == False, "expected wildcard to be set"


def parse_exit_code_test():
    print "Testing util.parse_exit_code()..."
    assert 54 == parse_exit_code('hello_error54_world')
    assert 1 == parse_exit_code('hello_error_world')
    assert 0 == parse_exit_code('hello_world')


def shell_quote_win_test():
    print "Testing util.shell_quote_win()..."
    assert 'simple' == shell_quote_win('simple')
    assert 'roof/\\isoprojection' == shell_quote_win('roof/\\isoprojection')
    assert '"with space"' == shell_quote_win('with space')
    assert '"embedded""quote"' == shell_quote_win('embedded"quote')
    assert '"a""b""""c\\d\\\\""e\\\\\\\\"' == shell_quote_win(
        'a"b""c\\d\\"e\\\\')


def parse_unit_test_output_test():
    print "Testing util.parse_unit_test_output()..."
    # This is an example of a successful unit test output.
    output = open(
        os.path.join(util.root_path, "tools/testdata/unit_test_output1.txt"))
    (actual, expected) = util.parse_unit_test_output(output, False)
    assert actual == 96
    assert expected == 96

    # This is an example of a silently dying unit test.
    output = open(
        os.path.join(util.root_path, "tools/testdata/unit_test_output2.txt"))
    (actual, expected) = util.parse_unit_test_output(output, False)
    assert actual == None
    assert expected == 96

    # This is an example of compiling before successful unit tests.
    output = open(
        os.path.join(util.root_path, "tools/testdata/unit_test_output3.txt"))
    (actual, expected) = util.parse_unit_test_output(output, False)
    assert actual == 96
    assert expected == 96

    # Check what happens on empty output.
    from StringIO import StringIO
    output = StringIO("\n\n\n")
    (actual, expected) = util.parse_unit_test_output(output, False)
    assert actual == None
    assert expected == None


def parse_wrk_output_test():
    print "Testing util.parse_wrk_output_test()..."
    f = open(os.path.join(util.root_path, "tools/testdata/wrk1.txt"))
    req_per_sec = util.parse_wrk_output(f.read())
    assert req_per_sec == 1837


def util_test():
    pattern_match_test()
    parse_exit_code_test()
    shell_quote_win_test()
    parse_unit_test_output_test()
    parse_wrk_output_test()


if __name__ == '__main__':
    util_test()
