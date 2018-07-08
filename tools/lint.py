#!/usr/bin/env python
# Does google-lint on c++ files and ts-lint on typescript files

import os
import subprocess

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = os.path.join(root_path, "third_party")
cpplint = os.path.join(third_party_path, "cpplint", "cpplint.py")
tslint = os.path.join(third_party_path, "node_modules", "tslint", "bin",
                      "tslint")


def run(args):
    print(" ".join(args))
    env = os.environ.copy()
    subprocess.check_call(args, env=env)


def main():
    os.chdir(root_path)
    run([
        "python", cpplint, "--filter=-build/include_subdir",
        "--repository=src", "--extensions=cc,h", "--recursive", "src/."
    ])
    run(["node", tslint, "-p", ".", "--exclude", "js/msg_generated.ts"])


if __name__ == "__main__":
    main()
