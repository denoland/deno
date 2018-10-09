# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
from subprocess import PIPE, Popen
import sys
from time import sleep

from util import build_path, executable_suffix, green_ok


class Repl(object):
    def __init__(self, deno_exe):
        self.deno_exe = deno_exe
        self.warm_up()

    def input(self, *lines, **kwargs):
        exit_ = kwargs.pop("exit", True)
        p = Popen([self.deno_exe], stdout=PIPE, stderr=PIPE, stdin=PIPE)
        try:
            for line in lines:
                p.stdin.write(line.encode("utf-8") + b'\n')
            if exit_:
                p.stdin.write(b'deno.exit(0)\n')
            else:
                sleep(1)  # wait to be killed by js
            out, err = p.communicate()
        except Exception as e:  # Should this be CalledProcessError?
            p.kill()
            p.wait()
            raise
        retcode = p.poll()
        # Ignore Windows CRLF (\r\n).
        return out.replace('\r\n', '\n'), err.replace('\r\n', '\n'), retcode

    def warm_up(self):
        # This may output an error message about the history file (ignore it).
        self.input("")

    def test_function(self):
        out, err, code = self.input("deno.writeFileSync")
        assertEqual(out, '[Function: writeFileSync]\n')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_console_log(self):
        out, err, code = self.input("console.log('hello')", "'world'")
        assertEqual(out, 'hello\nundefined\nworld\n')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_variable(self):
        out, err, code = self.input("var a = 123;", "a")
        assertEqual(out, 'undefined\n123\n')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_settimeout(self):
        out, err, code = self.input(
            "setTimeout(() => { console.log('b'); deno.exit(0); }, 10)",
            "'a'",
            exit=False)
        assertEqual(out, '1\na\nb\n')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_reference_error(self):
        out, err, code = self.input("not_a_variable")
        assertEqual(out, '')
        assertEqual(err, 'ReferenceError: not_a_variable is not defined\n')
        assertEqual(code, 0)

    def test_syntax_error(self):
        out, err, code = self.input("syntax error")
        assertEqual(out, '')
        assertEqual(err, "SyntaxError: Unexpected identifier\n")
        assertEqual(code, 0)

    def test_type_error(self):
        out, err, code = self.input("console()")
        assertEqual(out, '')
        assertEqual(err, 'TypeError: console is not a function\n')
        assertEqual(code, 0)

    def test_exit_command(self):
        out, err, code = self.input(".exit", "'ignored'", exit=False)
        assertEqual(out, '')
        assertEqual(err, '')
        assertEqual(code, 0)

    def run(self):
        print('repl_test.py')
        test_names = [name for name in dir(self) if name.startswith("test_")]
        for t in test_names:
            self.__getattribute__(t)()
            sys.stdout.write(".")
            sys.stdout.flush()
        print(' {}\n'.format(green_ok()))


def assertEqual(left, right):
    if left != right:
        raise AssertionError("{} != {}".format(repr(left), repr(right)))


def repl_tests(deno_exe):
    Repl(deno_exe).run()


if __name__ == "__main__":
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    repl_tests(deno_exe)
