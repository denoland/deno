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


if __name__ == "__main__":
    run_tests()
