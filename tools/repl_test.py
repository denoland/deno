# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
from subprocess import CalledProcessError, PIPE, Popen
import sys
import time

from test_util import DenoTestCase, run_tests


class TestRepl(DenoTestCase):
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
                p.stdin.write(b'Deno.exit(0)\n')
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

    def test_console_log(self):
        out, err, code = self.input("console.log('hello')", "'world'")
        self.assertEqual(out, 'hello\nundefined\nworld\n')
        self.assertEqual(err, '')
        self.assertEqual(code, 0)

    def test_exit_command(self):
        out, err, code = self.input("exit", "'ignored'", exit=False)
        self.assertEqual(out, '')
        self.assertEqual(err, '')
        self.assertEqual(code, 0)

    def test_help_command(self):
        out, err, code = self.input("help")
        expectedOut = '\n'.join([
            "exit    Exit the REPL",
            "help    Print this help message",
            "",
        ])
        self.assertEqual(out, expectedOut)
        self.assertEqual(err, '')
        self.assertEqual(code, 0)

    def test_function(self):
        out, err, code = self.input("Deno.writeFileSync")
        self.assertEqual(out, '[Function: writeFileSync]\n')
        self.assertEqual(err, '')
        self.assertEqual(code, 0)

    def test_multiline(self):
        out, err, code = self.input("(\n1 + 2\n)")
        self.assertEqual(out, '3\n')
        self.assertEqual(err, '')
        self.assertEqual(code, 0)

    # This should print error instead of wait for input
    def test_eval_unterminated(self):
        out, err, code = self.input("eval('{')")
        self.assertEqual(out, '')
        assert "Unexpected end of input" in err
        self.assertEqual(code, 0)

    def test_reference_error(self):
        out, err, code = self.input("not_a_variable")
        self.assertEqual(out, '')
        assert "not_a_variable is not defined" in err
        self.assertEqual(code, 0)

    # def test_set_timeout(self):
    #     out, err, code = self.input(
    #         "setTimeout(() => { console.log('b'); Deno.exit(0); }, 1)",
    #         "'a'",
    #         exit=False)
    #     self.assertEqual(out, '1\na\nb\n')
    #     self.assertEqual(err, '')
    #     self.assertEqual(code, 0)

    # def test_set_timeout_interlaced(self):
    #     out, err, code = self.input(
    #         "setTimeout(() => console.log('a'), 1)",
    #         "setTimeout(() => console.log('b'), 6)",
    #         sleep=0.8)
    #     self.assertEqual(out, '1\n2\na\nb\n')
    #     self.assertEqual(err, '')
    #     self.assertEqual(code, 0)

    # def test_async_op(self):
    #     out, err, code = self.input(
    #         "fetch('http://localhost:4545/tests/001_hello.js')" +
    #         ".then(res => res.text()).then(console.log)",
    #         sleep=1)
    #     self.assertEqual(out, 'Promise {}\nconsole.log("Hello World");\n\n')
    #     self.assertEqual(err, '')
    #     self.assertEqual(code, 0)

    def test_syntax_error(self):
        out, err, code = self.input("syntax error")
        self.assertEqual(out, '')
        assert "Unexpected identifier" in err
        self.assertEqual(code, 0)

    def test_type_error(self):
        out, err, code = self.input("console()")
        self.assertEqual(out, '')
        assert "console is not a function" in err
        self.assertEqual(code, 0)

    def test_variable(self):
        out, err, code = self.input("var a = 123;", "a")
        self.assertEqual(out, 'undefined\n123\n')
        self.assertEqual(err, '')
        self.assertEqual(code, 0)

    def test_lexical_scoped_variable(self):
        out, err, code = self.input("let a = 123;", "a")
        self.assertEqual(out, 'undefined\n123\n')
        self.assertEqual(err, '')
        self.assertEqual(code, 0)


if __name__ == "__main__":
    run_tests()
