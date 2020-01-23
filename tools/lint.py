#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# Does google-lint on c++ files and ts-lint on typescript files

import os
import sys
from util import enable_ansi_colors, git_ls_files, root_path, run
from util import third_party_path
from third_party import python_env


def main():
    enable_ansi_colors()
    os.chdir(root_path)
    eslint()
    pylint()


def eslint():
    print "eslint"
    script = os.path.join(third_party_path, "node_modules", "eslint", "bin",
                          "eslint")
    # Find all *directories* in the main repo that contain .ts/.js files.
    source_files = git_ls_files(root_path, [
        "*.js", "*.ts", ":!:std/prettier/vendor/*", ":!:std/**/testdata/*",
        ":!:std/**/node_modules/*", ":!:cli/compilers/*"
    ])
    source_dirs = set([os.path.dirname(f) for f in source_files])
    # Within the source dirs, eslint does its own globbing, taking into account
    # the exclusion rules listed in '.eslintignore'.
    source_globs = ["%s/*.{js,ts}" % d for d in source_dirs]
    run(["node", script, "--max-warnings=0", "--"] + source_globs,
        shell=False,
        quiet=True)


def pylint():
    print "pylint"
    script = os.path.join(third_party_path, "python_packages", "pylint")
    rcfile = os.path.join(root_path, "tools", "pylintrc")
    source_files = git_ls_files(root_path, ["*.py"])
    run([sys.executable, script, "--rcfile=" + rcfile, "--"] + source_files,
        env=python_env(),
        shell=False,
        quiet=True)


if __name__ == "__main__":
    sys.exit(main())
