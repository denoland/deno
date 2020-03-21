import os
import sys

from test_util import DenoTestCase, run_tests
from util import executable_suffix, tests_path, run, run_output


class TestTarget(DenoTestCase):
    @staticmethod
    def check_exists(filename):
        if not os.path.exists(filename):
            print "Required target doesn't exist:", filename
            sys.exit(1)

    def test_executable_exists(self):
        self.check_exists(self.deno_exe)

    def _test(self, executable):
        "Test executable runs and exits with code 0."
        bin_file = os.path.join(self.build_dir, executable + executable_suffix)
        self.check_exists(bin_file)
        run([bin_file], quiet=True)

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
            "cli/tests/exec_path.ts"
        ]
        result = run_output(cmd, quiet=True)
        print "exec_path", result
        self.assertEqual(result.code, 0)
        if os.name == "nt":
            # When running in github actions, the windows drive letter of the
            # executable path reported by deno has a different case than the one
            # reported by python.
            assert self.deno_exe.upper() in result.out.strip().upper()
            assert self.deno_exe[1:] in result.out.strip()
        else:
            assert self.deno_exe in result.out.strip()


if __name__ == "__main__":
    run_tests()
