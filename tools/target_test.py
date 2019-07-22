import os
import sys

from test_util import DenoTestCase, run_tests
from util import executable_suffix, tests_path, run, run_output


class TestTarget(DenoTestCase):
    @staticmethod
    def check_exists(filename):
        if not os.path.exists(filename):
            print "Required target doesn't exist:", filename
            print "Run ./tools/build.py"
            sys.exit(1)

    def is_gn_build(self):
        return os.path.exists(
            os.path.join(self.build_dir, "cli_test" + executable_suffix))

    def test_executable_exists(self):
        self.check_exists(self.deno_exe)

    def _test(self, executable):
        "Test executable runs and exits with code 0."
        bin_file = os.path.join(self.build_dir, executable + executable_suffix)
        self.check_exists(bin_file)
        run([bin_file], quiet=True)

    def test_libdeno(self):
        # TODO(ry) libdeno_test is not being run under cargo build!!!
        # This must be fixed before landing.
        if self.is_gn_build():
            self._test("libdeno_test")

    def test_rust_unit_tests(self):
        if self.is_gn_build():
            self._test("cli_test")
            self._test("deno_core_test")
            self._test("deno_core_http_bench_test")
        else:
            args = ["cargo", "test", "--examples", "--all-targets", "-vv"]
            from test_util import TEST_ARGS
            if TEST_ARGS.release:
                args += ["--release"]
            run(args, quiet=True)

    def test_no_color(self):
        t = os.path.join(tests_path, "no_color.js")
        result = run_output([self.deno_exe, "run", t],
                            merge_env={"NO_COLOR": "1"},
                            quiet=True)
        assert result.out.strip() == "noColor true"
        t = os.path.join(tests_path, "no_color.js")
        result = run_output([self.deno_exe, "run", t], quiet=True)
        assert result.out.strip() == "noColor false"

    def test_exec_path(self):
        cmd = [
            self.deno_exe, "run", "--allow-run", "--allow-env",
            "tests/exec_path.ts"
        ]
        result = run_output(cmd, quiet=True)
        print "exec_path", result.code
        print result.out
        print result.err
        assert self.deno_exe in result.out.strip()
        self.assertEqual(result.code, 0)


if __name__ == "__main__":
    run_tests()
