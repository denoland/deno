#!/usr/bin/env python
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import cStringIO
import find_incompatible_glibc_symbols

NM_DATA = """\
0000000000000001 W expf@GLIBC_2.2.5
0000000000000002 W expf@@GLIBC_2.27
0000000000000003 W foo@@GLIBC_2.2.5
0000000000000004 W bar@GLIBC_2.2.5
0000000000000005 W baz@GLIBC_2.2.5
0000000000000006 T foo2@GLIBC_2.2
0000000000000007 T foo2@GLIBC_2.3
0000000000000008 T foo2@GLIBC_2.30
0000000000000009 T foo2@@GLIBC_2.31
000000000000000a T bar2@GLIBC_2.30
000000000000000b T bar2@@GLIBC_2.31
000000000000000c T baz2@GLIBC_2.2
000000000000000d T baz2@@GLIBC_2.3
"""

EXPECTED_REPLACEMENTS = [
    '__asm__(".symver expf, expf@GLIBC_2.2.5");',
    '__asm__(".symver foo2, foo2@GLIBC_2.3");',
]

nm_file = cStringIO.StringIO()
nm_file.write(NM_DATA)
nm_file.seek(0)

assert (
    EXPECTED_REPLACEMENTS == find_incompatible_glibc_symbols.get_replacements(
        nm_file, [2, 17]))
