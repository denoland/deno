# Copyright 2018 the Deno authors. All rights reserved. MIT license.
from util import pattern_match, parse_exit_code


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
        assert expected == actual, "expected %s for\nExpected:\n%s\nTo equal actual:\n%s" % (
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


def util_test():
    pattern_match_test()
    parse_exit_code_test()


if __name__ == '__main__':
    util_test()
