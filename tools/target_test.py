import os
import sys

from test_util import DenoTestCase, run_tests
from util import executable_suffix, run, tests_path, run_output


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
        run([bin_file])

    def test_libdeno(self):
        self._test("libdeno_test")

    def test_cli(self):
        self._test("cli_test")

    def test_core(self):
        self._test("deno_core_test")

    def test_core_http_benchmark(self):
        self._test("deno_core_http_bench_test")

    def test_ts_library_builder(self):
        run([
            "node", "./node_modules/.bin/ts-node", "--project",
            "tools/ts_library_builder/tsconfig.json",
            "tools/ts_library_builder/test.ts"
        ])

    def test_no_color(self):
        t = os.path.join(tests_path, "no_color.js")
        output = run_output([self.deno_exe, "run", t],
                            merge_env={"NO_COLOR": "1"})
        assert output.strip() == "noColor true"
        t = os.path.join(tests_path, "no_color.js")
        output = run_output([self.deno_exe, "run", t])
        assert output.strip() == "noColor false"

    def test_exec_path(self):
        cmd = [self.deno_exe, "run", "tests/exec_path.ts"]
        output = run_output(cmd)
        assert self.deno_exe in output.strip()


if __name__ == "main":
    run_tests()
