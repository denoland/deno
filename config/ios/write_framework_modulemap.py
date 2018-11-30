# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
import sys

def Main(framework):
  # Find the name of the binary based on the part before the ".framework".
  binary = os.path.basename(framework).split('.')[0]
  module_path = os.path.join(framework, 'Modules');
  if not os.path.exists(module_path):
    os.mkdir(module_path)
  module_template = 'framework module %s {\n' \
                    '  umbrella header "%s.h"\n' \
                    '\n' \
                    '  export *\n' \
                    '  module * { export * }\n' \
                    '}\n' % (binary, binary)

  module_file = open(os.path.join(module_path, 'module.modulemap'), 'w')
  module_file.write(module_template)
  module_file.close()

if __name__ == '__main__':
  Main(sys.argv[1])
