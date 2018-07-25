#!/usr/bin/env python
import third_party

third_party.fix_symlinks()

third_party.download_gn()
third_party.download_clang()
