# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import array
import difflib
import distutils.dir_util
import filecmp
import operator
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import uuid


def ZapTimestamp(filename):
  contents = open(filename, 'rb').read()
  # midl.exe writes timestamp 2147483647 (2^31 - 1) as creation date into its
  # outputs, but using the local timezone.  To make the output timezone-
  # independent, replace that date with a fixed string of the same length.
  # Also blank out the minor version number.
  if filename.endswith('.tlb'):
    # See https://chromium-review.googlesource.com/c/chromium/src/+/693223 for
    # a fairly complete description of the .tlb binary format.
    # TLB files start with a 54 byte header. Offset 0x20 stores how many types
    # are defined in the file, and the header is followed by that many uint32s.
    # After that, 15 section headers appear.  Each section header is 16 bytes,
    # starting with offset and length uint32s.
    # Section 12 in the file contains custom() data. custom() data has a type
    # (int, string, etc).  Each custom data chunk starts with a uint16_t
    # describing its type.  Type 8 is string data, consisting of a uint32_t
    # len, followed by that many data bytes, followed by 'W' bytes to pad to a
    # 4 byte boundary.  Type 0x13 is uint32 data, followed by 4 data bytes,
    # followed by two 'W' to pad to a 4 byte boundary.
    # The custom block always starts with one string containing "Created by
    # MIDL version 8...", followed by one uint32 containing 0x7fffffff,
    # followed by another uint32 containing the MIDL compiler version (e.g.
    # 0x0801026e for v8.1.622 -- 0x26e == 622).  These 3 fields take 0x54 bytes.
    # There might be more custom data after that, but these 3 blocks are always
    # there for file-level metadata.
    # All data is little-endian in the file.
    assert contents[0:8] == 'MSFT\x02\x00\x01\x00'
    ntypes, = struct.unpack_from('<I', contents, 0x20)
    custom_off, custom_len = struct.unpack_from(
        '<II', contents, 0x54 + 4*ntypes + 11*16)
    assert custom_len >= 0x54
    # First: Type string (0x8), followed by 0x3e characters.
    assert contents[custom_off:custom_off+6] == '\x08\x00\x3e\x00\x00\x00'
    assert re.match(
        'Created by MIDL version 8\.\d\d\.\d{4} at ... Jan 1. ..:..:.. 2038\n',
        contents[custom_off+6:custom_off+6+0x3e])
    # Second: Type uint32 (0x13) storing 0x7fffffff (followed by WW / 0x57 pad)
    assert contents[custom_off+6+0x3e:custom_off+6+0x3e+8] == \
        '\x13\x00\xff\xff\xff\x7f\x57\x57'
    # Third: Type uint32 (0x13) storing MIDL compiler version.
    assert contents[custom_off+6+0x3e+8:custom_off+6+0x3e+8+2] == '\x13\x00'
    # Replace "Created by" string with fixed string, and fixed MIDL version with
    # 8.1.622 always.
    contents = (contents[0:custom_off+6] +
        'Created by MIDL version 8.xx.xxxx at a redacted point in time\n' +
        # uint32 (0x13) val 0x7fffffff, WW, uint32 (0x13), val 0x0801026e, WW
        '\x13\x00\xff\xff\xff\x7f\x57\x57\x13\x00\x6e\x02\x01\x08\x57\x57' +
        contents[custom_off + 0x54:])
  else:
    contents = re.sub(
        'File created by MIDL compiler version 8\.\d\d\.\d{4} \*/\r\n'
        '/\* at ... Jan 1. ..:..:.. 2038',
        'File created by MIDL compiler version 8.xx.xxxx */\r\n'
        '/* at a redacted point in time',
        contents)
    contents = re.sub(
        '    Oicf, W1, Zp8, env=(.....) \(32b run\), '
        'target_arch=(AMD64|X86) 8\.\d\d\.\d{4}',
        '    Oicf, W1, Zp8, env=\\1 (32b run), target_arch=\\2 8.xx.xxxx',
        contents)
    # TODO(thakis): If we need more hacks than these, try to verify checked-in
    # outputs when we're using the hermetic toolchain.
    # midl.exe older than 8.1.622 omit '//' after #endif, fix that:
    contents = contents.replace('#endif !_MIDL_USE_GUIDDEF_',
                                '#endif // !_MIDL_USE_GUIDDEF_')
    # midl.exe puts the midl version into code in one place.  To have
    # predictable output, lie about the midl version if it's not 8.1.622.
    # This is unfortunate, but remember that there's beauty too in imperfection.
    contents = contents.replace('0x801026c, /* MIDL Version 8.1.620 */',
                                '0x801026e, /* MIDL Version 8.1.622 */')
  open(filename, 'wb').write(contents)


def overwrite_cls_guid_h(h_file, dynamic_guid):
  contents = open(h_file, 'rb').read()
  contents = re.sub('class DECLSPEC_UUID\("[^"]*"\)',
                    'class DECLSPEC_UUID("%s")' % str(dynamic_guid), contents)
  open(h_file, 'wb').write(contents)


def overwrite_cls_guid_iid(iid_file, dynamic_guid):
  contents = open(iid_file, 'rb').read()
  hexuuid = '0x%08x,0x%04x,0x%04x,' % dynamic_guid.fields[0:3]
  hexuuid += ','.join('0x%02x' % ord(b) for b in dynamic_guid.bytes[8:])
  contents = re.sub(r'MIDL_DEFINE_GUID\(CLSID, ([^,]*),[^)]*\)',
                    r'MIDL_DEFINE_GUID(CLSID, \1,%s)' % hexuuid, contents)
  open(iid_file, 'wb').write(contents)


def overwrite_cls_guid_tlb(tlb_file, dynamic_guid):
  # See ZapTimestamp() for a short overview of the .tlb format.  The 1st
  # section contains type descriptions, and the first type should be our
  # coclass.  It points to the type's GUID in section 6, the GUID section.
  contents = open(tlb_file, 'rb').read()
  assert contents[0:8] == 'MSFT\x02\x00\x01\x00'
  ntypes, = struct.unpack_from('<I', contents, 0x20)
  type_off, type_len = struct.unpack_from('<II', contents, 0x54 + 4*ntypes)
  assert ord(contents[type_off]) == 0x25, "expected coclass"
  guidind = struct.unpack_from('<I', contents, type_off + 0x2c)[0]
  guid_off, guid_len = struct.unpack_from(
      '<II', contents, 0x54 + 4*ntypes + 5*16)
  assert guidind + 14 <= guid_len
  contents = array.array('c', contents)
  struct.pack_into('<IHH8s', contents, guid_off + guidind,
                   *(dynamic_guid.fields[0:3] + (dynamic_guid.bytes[8:],)))
  # The GUID is correct now, but there's also a GUID hashtable in section 5.
  # Need to recreate that too.  Since the hash table uses chaining, it's
  # easiest to recompute it from scratch rather than trying to patch it up.
  hashtab = [0xffffffff] * (0x80 / 4)
  for guidind in range(guid_off, guid_off + guid_len, 24):
    guidbytes, typeoff, nextguid = struct.unpack_from(
        '<16sII', contents, guidind)
    words = struct.unpack('<8H', guidbytes)
    # midl seems to use the following simple hash function for GUIDs:
    guidhash = reduce(operator.xor, [w for w in words]) % (0x80 / 4)
    nextguid = hashtab[guidhash]
    struct.pack_into('<I', contents, guidind + 0x14, nextguid)
    hashtab[guidhash] = guidind - guid_off
  hash_off, hash_len = struct.unpack_from(
      '<II', contents, 0x54 + 4*ntypes + 4*16)
  for i, hashval in enumerate(hashtab):
    struct.pack_into('<I', contents, hash_off + 4*i, hashval)
  open(tlb_file, 'wb').write(contents)


def overwrite_cls_guid(h_file, iid_file, tlb_file, dynamic_guid):
  # Fix up GUID in .h, _i.c, and .tlb.  This currently assumes that there's
  # only one coclass in the idl file, and that that's the type with the
  # dynamic type.
  overwrite_cls_guid_h(h_file, dynamic_guid)
  overwrite_cls_guid_iid(iid_file, dynamic_guid)
  overwrite_cls_guid_tlb(tlb_file, dynamic_guid)


def main(arch, outdir, dynamic_guid, tlb, h, dlldata, iid, proxy, idl, *flags):
  # Copy checked-in outputs to final location.
  THIS_DIR = os.path.abspath(os.path.dirname(__file__))
  source = os.path.join(THIS_DIR, '..', '..', '..',
      'third_party', 'win_build_output', outdir.replace('gen/', 'midl/'))
  if os.path.isdir(os.path.join(source, os.path.basename(idl))):
    source = os.path.join(source, os.path.basename(idl))
  source = os.path.join(source, arch.split('.')[1])  # Append 'x86' or 'x64'.
  source = os.path.normpath(source)
  distutils.dir_util.copy_tree(source, outdir, preserve_times=False)
  if dynamic_guid != 'none':
    overwrite_cls_guid(os.path.join(outdir, h),
                       os.path.join(outdir, iid),
                       os.path.join(outdir, tlb),
                       uuid.UUID(dynamic_guid))

  # On non-Windows, that's all we can do.
  if sys.platform != 'win32':
    return 0

  # On Windows, run midl.exe on the input and check that its outputs are
  # identical to the checked-in outputs (after possibly replacing their main
  # class guid).
  tmp_dir = tempfile.mkdtemp()
  delete_tmp_dir = True

  # Read the environment block from the file. This is stored in the format used
  # by CreateProcess. Drop last 2 NULs, one for list terminator, one for
  # trailing vs. separator.
  env_pairs = open(arch).read()[:-2].split('\0')
  env_dict = dict([item.split('=', 1) for item in env_pairs])

  args = ['midl', '/nologo'] + list(flags) + [
      '/out', tmp_dir,
      '/tlb', tlb,
      '/h', h,
      '/dlldata', dlldata,
      '/iid', iid,
      '/proxy', proxy,
      idl]
  try:
    popen = subprocess.Popen(args, shell=True, env=env_dict,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    out, _ = popen.communicate()
    # Filter junk out of stdout, and write filtered versions. Output we want
    # to filter is pairs of lines that look like this:
    # Processing C:\Program Files (x86)\Microsoft SDKs\...\include\objidl.idl
    # objidl.idl
    lines = out.splitlines()
    prefixes = ('Processing ', '64 bit Processing ')
    processing = set(os.path.basename(x)
                     for x in lines if x.startswith(prefixes))
    for line in lines:
      if not line.startswith(prefixes) and line not in processing:
        print line
    if popen.returncode != 0:
      return popen.returncode

    for f in os.listdir(tmp_dir):
      ZapTimestamp(os.path.join(tmp_dir, f))

    # Now compare the output in tmp_dir to the copied-over outputs.
    diff = filecmp.dircmp(tmp_dir, outdir)
    if diff.diff_files:
      print 'midl.exe output different from files in %s, see %s' \
          % (outdir, tmp_dir)
      for f in diff.diff_files:
        if f.endswith('.tlb'): continue
        fromfile = os.path.join(outdir, f)
        tofile = os.path.join(tmp_dir, f)
        print ''.join(difflib.unified_diff(open(fromfile, 'U').readlines(),
                                           open(tofile, 'U').readlines(),
                                           fromfile, tofile))
      delete_tmp_dir = False
      print 'To rebaseline:'
      print '  copy /y %s\* %s' % (tmp_dir, source)
      sys.exit(1)
    return 0
  finally:
    if os.path.exists(tmp_dir) and delete_tmp_dir:
      shutil.rmtree(tmp_dir)


if __name__ == '__main__':
  sys.exit(main(*sys.argv[1:]))
