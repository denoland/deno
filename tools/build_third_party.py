#!/usr/bin/env python
# This script updates the third party dependencies of deno.
# - Get Depot Tools and make sure it's in your path.
#   http://commondatastorage.googleapis.com/chrome-infra-docs/flat/depot_tools/docs/html/depot_tools_tutorial.html#_setting_up
# - You need yarn installed as well.
#   https://yarnpkg.com/lang/en/docs/install/
# Use //gclient_config.py to modify the git deps.
# Use //js/package.json to modify the npm deps.

import os
import subprocess
import argparse

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = os.path.join(root_path, "third_party")
script_name = "build_third_party.py"

parser = argparse.ArgumentParser(description="""
This script updates the third party dependencies of deno.
""")
parser.parse_args()

def main():
    os.chdir(third_party_path)
    run(["gclient", "sync", "--no-history"])
    run(["yarn"])
    print "Done (" + script_name + ")"

def run(args):
    print " ".join(args)
    env = os.environ.copy()
    subprocess.check_call(args, env=env)

if '__main__' == __name__:
    main()
