import os
import sys

from test_util import DenoTestCase, run_tests
from util import executable_suffix, tests_path, run, run_output


# In the ninja/gn we build and test individually libdeno_test, cli_test,
# deno_core_test, deno_core_http_bench_test. When building with cargo, however
# we just run "cargo test".
# This is hacky but is only temporarily here until the ninja/gn build is
# removed.
def is_cargo_test():
    return "CARGO_TEST" in os.environ


class TestTarget(DenoTestCase):
    @staticmethod
    def check_exists(filename):
        if not os.path.exists(filename):
            print "Required target doesn't exist:", filename
            print "Run ./tools/build.py"
            sys.exit(1)

    def test_executable_exists(self):
        self.check_exists(self.deno_exe)

    def _test(self, executable):
        "Test executable runs and exits with code 0."
        bin_file = os.path.join(self.build_dir, executable + executable_suffix)
        self.check_exists(bin_file)
        run([bin_file], quiet=True)

    def test_cargo_test(self):
        if is_cargo_test():
            cargo_test = ["cargo", "test", "--all", "--locked"]
            if os.environ["DENO_BUILD_MODE"] == "release":
                run(cargo_test + ["--release"])
            else:
                run(cargo_test)

    def test_libdeno(self):
        if not is_cargo_test():
            self._test("libdeno_test")

    def test_cli(self):
        if not is_cargo_test():
            self._test("cli_test")

    def test_core(self):
        if not is_cargo_test():
            self._test("deno_core_test")

    def test_core_http_benchmark(self):
        if not is_cargo_test():
            self._test("deno_core_http_bench_test")

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
