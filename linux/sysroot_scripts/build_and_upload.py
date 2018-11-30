#!/usr/bin/env python
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Automates running BuildPackageLists, BuildSysroot, and
UploadSysroot for each supported arch of each sysroot creator.
"""

import glob
import hashlib
import json
import multiprocessing
import os
import re
import string
import subprocess
import sys

def run_script(args):
  fnull = open(os.devnull, 'w')
  subprocess.check_call(args, stdout=fnull, stderr=fnull)

def sha1sumfile(filename):
  sha1 = hashlib.sha1()
  with open(filename, 'rb') as f:
    while True:
      data = f.read(65536)
      if not data:
        break
      sha1.update(data)
  return sha1.hexdigest()

def get_proc_output(args):
  return subprocess.check_output(args).strip()

def build_and_upload(script_path, distro, release, arch, lock):
  # TODO(thomasanderson):  Find out which revision 'git-cl upload' uses to
  # calculate the diff against and use that instead of HEAD.
  script_dir = os.path.dirname(os.path.realpath(__file__))
  revision = get_proc_output(['git', '-C', script_dir, 'rev-parse', 'HEAD'])

  run_script([script_path, 'UpdatePackageLists%s' % arch])
  run_script([script_path, 'BuildSysroot%s' % arch])
  run_script([script_path, 'UploadSysroot%s' % arch, revision])

  tarball = '%s_%s_%s_sysroot.tar.xz' % (distro, release, arch.lower())
  tarxz_path = os.path.join(script_dir, "..", "..", "..", "out",
                            "sysroot-build", release, tarball)
  sha1sum = sha1sumfile(tarxz_path)
  sysroot_dir = '%s_%s_%s-sysroot' % (distro, release, arch.lower())

  sysroot_metadata = {
      'Revision': revision,
      'Tarball': tarball,
      'Sha1Sum': sha1sum,
      'SysrootDir': sysroot_dir
  }
  with lock:
    with open(os.path.join(script_dir, 'sysroots.json'), 'rw+') as f:
      sysroots = json.load(f)
      sysroots["%s_%s" % (release, arch.lower())] = sysroot_metadata
      f.seek(0)
      f.truncate()
      f.write(json.dumps(sysroots, sort_keys=True, indent=4,
                         separators=(',', ': ')))
      f.write('\n')

def main():
  script_dir = os.path.dirname(os.path.realpath(__file__))
  procs = []
  lock = multiprocessing.Lock()
  for filename in glob.glob(os.path.join(script_dir, 'sysroot-creator-*.sh')):
    script_path = os.path.join(script_dir, filename)
    distro = get_proc_output([script_path, 'PrintDistro'])
    release = get_proc_output([script_path, 'PrintRelease'])
    architectures = get_proc_output([script_path, 'PrintArchitectures'])
    for arch in architectures.split('\n'):
      proc = multiprocessing.Process(target=build_and_upload,
                                     args=(script_path, distro, release, arch,
                                           lock))
      procs.append(("%s %s (%s)" % (distro, release, arch), proc))
      proc.start()
  for _, proc in procs:
    proc.join()

  print "SYSROOT CREATION SUMMARY"
  failures = 0
  for name, proc in procs:
    if proc.exitcode:
      failures += 1
    status = "FAILURE" if proc.exitcode else "SUCCESS"
    print "%s sysroot creation\t%s" % (name, status)
  return failures

if __name__ == '__main__':
  sys.exit(main())
