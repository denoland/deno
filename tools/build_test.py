#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import sys
from build import main as build
from test import main as test

build(sys.argv)
test(sys.argv)
