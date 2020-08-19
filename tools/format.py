#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
import sys
from third_party import get_prebuilt_tool_path
from util import root_path
from util import run

def main():
    os.chdir(root_path)
    dprint()


def dprint():
    executable_path = get_prebuilt_tool_path("dprint")
    command = [executable_path, "fmt"]
    run(command, shell=False, quiet=True)


if __name__ == "__main__":
    sys.exit(main())
