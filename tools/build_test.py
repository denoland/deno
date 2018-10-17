#!/usr/bin/env python
import sys
from build import main as build
from test import main as test

build(sys.argv)
test(sys.argv)
