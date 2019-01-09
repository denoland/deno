# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
from subprocess import CalledProcessError, PIPE, Popen
import sys
import time

from util import build_path, executable_suffix, green_ok


class Repl(object):
    def __init__(self, deno_exe):
        self.deno_exe = deno_exe
        self._warm_up()

    def _warm_up(self):
        # This may output an error message about the history file (ignore it).
        self.input("")

    def input(self, *lines, **kwargs):
        exit_ = kwargs.pop("exit", True)
        sleep_ = kwargs.pop("sleep", 0)
        p = Popen([self.deno_exe], stdout=PIPE, stderr=PIPE, stdin=PIPE)
        try:
            # Note: The repl takes a >100ms until it's ready.
            time.sleep(sleep_)
            for line in lines:
                p.stdin.write(line.encode("utf-8") + b'\n')
                p.stdin.flush()
                time.sleep(sleep_)
            if exit_:
                p.stdin.write(b'deno.exit(0)\n')
            else:
                time.sleep(1)  # wait to be killed by js
            out, err = p.communicate()
        except CalledProcessError as e:
            p.kill()
            p.wait()
            raise e
        retcode = p.poll()
        # Ignore Windows CRLF (\r\n).
        return out.replace('\r\n', '\n'), err.replace('\r\n', '\n'), retcode

    def run(self):
        print('repl_test.py')
        test_names = [name for name in dir(self) if name.startswith("test_")]
        for t in test_names:
            self.__getattribute__(t)()
            sys.stdout.write(".")
            sys.stdout.flush()
        print(' {}\n'.format(green_ok()))

    def test_console_log(self):
        out, err, code = self.input("console.log('hello')", "'world'")
        assertEqual(out, 'hello\nundefined\nworld\n')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_exit_command(self):
        out, err, code = self.input(".exit", "'ignored'", exit=False)
        assertEqual(out, '')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_function(self):
        out, err, code = self.input("deno.writeFileSync")
        assertEqual(out, '[Function: writeFileSync]\n')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_multiline(self):
        out, err, code = self.input("(\n1 + 2\n)")
        assertEqual(out, '3\n')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_reference_error(self):
        out, err, code = self.input("not_a_variable")
        assertEqual(out, '')
        assertEqual(err, 'ReferenceError: not_a_variable is not defined\n')
        assertEqual(code, 0)

    def test_set_timeout(self):
        out, err, code = self.input(
            "setTimeout(() => { console.log('b'); deno.exit(0); }, 10)",
            "'a'",
            exit=False)
        assertEqual(out, '1\na\nb\n')
        assertEqual(err, '')
        assertEqual(code, 0)

    def test_set_timeout_interlaced(self):
        out, err, code = self.input(
            "setTimeout(() => console.log('a'), 1000)",
            "setTimeout(() => console.log('b'), 600)",
            sleep=0.8)
        assertEqual(out, '1\n2\na\nb\n')
        assertEqual(err, '')
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

    def test_variable(self):
        out, err, code = self.input("var a = 123;", "a")
        assertEqual(out, 'undefined\n123\n')
        assertEqual(err, '')
        assertEqual(code, 0)


def assertEqual(left, right):
    if left != right:
        raise AssertionError("{} != {}".format(repr(left), repr(right)))


def repl_tests(deno_exe):
    Repl(deno_exe).run()


def main():
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    repl_tests(deno_exe)


if __name__ == "__main__":
    main()
