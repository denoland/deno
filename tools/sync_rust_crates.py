#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# There is a magic tool which has no documentation. It is used to update rust
# crates in third_party. https://github.com/piscisaureus/gnargo
import third_party
import util
util.enable_ansi_colors()
third_party.run_cargo()
