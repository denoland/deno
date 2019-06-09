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

    def test_executable_exists(self):
        self.check_exists(self.deno_exe)

    def _test(self, executable):
        "Test executable runs and exits with code 0."
        bin_file = os.path.join(self.build_dir, executable + executable_suffix)
        self.check_exists(bin_file)
        run([bin_file], quiet=True)

    def test_libdeno(self):
        self._test("libdeno_test")

    def test_cli(self):
        self._test("cli_test")

    def test_core(self):
        self._test("deno_core_test")

    def test_core_http_benchmark(self):
        self._test("deno_core_http_bench_test")

    def test_ts_library_builder(self):
        result = run_output([
            "node", "./node_modules/.bin/ts-node", "--project",
            "tools/ts_library_builder/tsconfig.json",
            "tools/ts_library_builder/test.ts"
        ],
                            quiet=True)
        self.assertEqual(result.code, 0)
        assert "ts_library_builder ok" in result.out

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
        cmd = [self.deno_exe, "run", "tests/exec_path.ts"]
        result = run_output(cmd, quiet=True)
        assert self.deno_exe in result.out.strip()
        self.assertEqual(result.code, 0)


if __name__ == "__main__":
    run_tests()
