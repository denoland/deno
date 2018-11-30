# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging
import unittest

from pylib.symbols import apk_native_libs

# Mock ELF-like data
MOCK_ELF_DATA = '\x7fELFFFFFFFFFFFFFFFF'

class MockApkZipInfo(object):
  """A mock ApkZipInfo class, returned by MockApkReaderFactory instances."""
  def __init__(self, filename, file_size, compress_size, file_offset,
               file_data):
    self.filename = filename
    self.file_size = file_size
    self.compress_size = compress_size
    self.file_offset = file_offset
    self._data = file_data

  def __repr__(self):
    """Convert to string for debugging."""
    return 'MockApkZipInfo["%s",size=%d,compressed=%d,offset=%d]' % (
        self.filename, self.file_size, self.compress_size, self.file_offset)

  def IsCompressed(self):
    """Returns True iff the entry is compressed."""
    return self.file_size != self.compress_size

  def IsElfFile(self):
    """Returns True iff the entry is an ELF file."""
    if not self._data or len(self._data) < 4:
      return False

    return self._data[0:4] == '\x7fELF'


class MockApkReader(object):
  """A mock ApkReader instance used during unit-testing.

  Do not use directly, but use a MockApkReaderFactory context, as in:

     with MockApkReaderFactory() as mock:
       mock.AddTestEntry(file_path, file_size, compress_size, file_data)
       ...

       # Actually returns the mock instance.
       apk_reader = apk_native_libs.ApkReader('/some/path.apk')
  """
  def __init__(self, apk_path='test.apk'):
    """Initialize instance."""
    self._entries = []
    self._fake_offset = 0
    self._path = apk_path

  def __enter__(self):
    return self

  def __exit__(self, *kwarg):
    self.Close()
    return

  @property
  def path(self):
    return self._path

  def AddTestEntry(self, filepath, file_size, compress_size, file_data):
    """Add a new entry to the instance for unit-tests.

    Do not call this directly, use the AddTestEntry() method on the parent
    MockApkReaderFactory instance.

    Args:
      filepath: archive file path.
      file_size: uncompressed file size in bytes.
      compress_size: compressed size in bytes.
      file_data: file data to be checked by IsElfFile()

    Note that file_data can be None, or that its size can be actually
    smaller than |compress_size| when used during unit-testing.
    """
    self._entries.append(MockApkZipInfo(filepath, file_size, compress_size,
                         self._fake_offset, file_data))
    self._fake_offset += compress_size

  def Close(self): # pylint: disable=no-self-use
    """Close this reader instance."""
    return

  def ListEntries(self):
    """Return a list of MockApkZipInfo instances for this input APK."""
    return self._entries

  def FindEntry(self, file_path):
    """Find the MockApkZipInfo instance corresponds to a given file path."""
    for entry in self._entries:
      if entry.filename == file_path:
        return entry
    raise KeyError('Could not find mock zip archive member for: ' + file_path)


class MockApkReaderTest(unittest.TestCase):

  def testEmpty(self):
    with MockApkReader() as reader:
      entries = reader.ListEntries()
      self.assertTrue(len(entries) == 0)
      with self.assertRaises(KeyError):
        reader.FindEntry('non-existent-entry.txt')

  def testSingleEntry(self):
    with MockApkReader() as reader:
      reader.AddTestEntry('some-path/some-file', 20000, 12345, file_data=None)
      entries = reader.ListEntries()
      self.assertTrue(len(entries) == 1)
      entry = entries[0]
      self.assertEqual(entry.filename, 'some-path/some-file')
      self.assertEqual(entry.file_size, 20000)
      self.assertEqual(entry.compress_size, 12345)
      self.assertTrue(entry.IsCompressed())

      entry2 = reader.FindEntry('some-path/some-file')
      self.assertEqual(entry, entry2)

  def testMultipleEntries(self):
    with MockApkReader() as reader:
      _ENTRIES = {
        'foo.txt': (1024, 1024, 'FooFooFoo'),
        'lib/bar/libcode.so': (16000, 3240, 1024, '\x7fELFFFFFFFFFFFF'),
      }
      for path, props in _ENTRIES.iteritems():
        reader.AddTestEntry(path, props[0], props[1], props[2])

      entries = reader.ListEntries()
      self.assertEqual(len(entries), len(_ENTRIES))
      for path, props in _ENTRIES.iteritems():
        entry = reader.FindEntry(path)
        self.assertEqual(entry.filename, path)
        self.assertEqual(entry.file_size, props[0])
        self.assertEqual(entry.compress_size, props[1])


class ApkNativeLibrariesTest(unittest.TestCase):

  def setUp(self):
    logging.getLogger().setLevel(logging.ERROR)

  def testEmptyApk(self):
    with MockApkReader() as reader:
      libs_map = apk_native_libs.ApkNativeLibraries(reader)
      self.assertTrue(libs_map.IsEmpty())
      self.assertEqual(len(libs_map.GetLibraries()), 0)
      lib_path, lib_offset = libs_map.FindLibraryByOffset(0)
      self.assertIsNone(lib_path)
      self.assertEqual(lib_offset, 0)

  def testSimpleApk(self):
    with MockApkReader() as reader:
      _MOCK_ENTRIES = [
        # Top-level library should be ignored.
        ('libfoo.so', 1000, 1000, MOCK_ELF_DATA, False),
        # Library not under lib/ should be ignored.
        ('badlib/test-abi/libfoo2.so', 1001, 1001, MOCK_ELF_DATA, False),
        # Library under lib/<abi>/ but without .so extension should be ignored.
        ('lib/test-abi/libfoo4.so.1', 1003, 1003, MOCK_ELF_DATA, False),
        # Library under lib/<abi>/ with .so suffix, but compressed -> ignored.
        ('lib/test-abi/libfoo5.so', 1004, 1003, MOCK_ELF_DATA, False),
        # First correct library
        ('lib/test-abi/libgood1.so', 1005, 1005, MOCK_ELF_DATA, True),
        # Second correct library: support sub-directories
        ('lib/test-abi/subdir/libgood2.so', 1006, 1006, MOCK_ELF_DATA, True),
        # Third correct library, no lib prefix required
        ('lib/test-abi/crazy.libgood3.so', 1007, 1007, MOCK_ELF_DATA, True),
      ]
      file_offsets = []
      prev_offset = 0
      for ent in _MOCK_ENTRIES:
        reader.AddTestEntry(ent[0], ent[1], ent[2], ent[3])
        file_offsets.append(prev_offset)
        prev_offset += ent[2]

      libs_map = apk_native_libs.ApkNativeLibraries(reader)
      self.assertFalse(libs_map.IsEmpty())
      self.assertEqual(libs_map.GetLibraries(), [
          'lib/test-abi/crazy.libgood3.so',
          'lib/test-abi/libgood1.so',
          'lib/test-abi/subdir/libgood2.so',
          ])

      BIAS = 10
      for mock_ent, file_offset in zip(_MOCK_ENTRIES, file_offsets):
        if mock_ent[4]:
          lib_path, lib_offset = libs_map.FindLibraryByOffset(
              file_offset + BIAS)
          self.assertEqual(lib_path, mock_ent[0])
          self.assertEqual(lib_offset, BIAS)


  def testMultiAbiApk(self):
    with MockApkReader() as reader:
      _MOCK_ENTRIES = [
        ('lib/abi1/libfoo.so', 1000, 1000, MOCK_ELF_DATA),
        ('lib/abi2/libfoo.so', 1000, 1000, MOCK_ELF_DATA),
      ]
      for ent in _MOCK_ENTRIES:
        reader.AddTestEntry(ent[0], ent[1], ent[2], ent[3])

      libs_map = apk_native_libs.ApkNativeLibraries(reader)
      self.assertFalse(libs_map.IsEmpty())
      self.assertEqual(libs_map.GetLibraries(), [
          'lib/abi1/libfoo.so', 'lib/abi2/libfoo.so'])

      lib1_name, lib1_offset = libs_map.FindLibraryByOffset(10)
      self.assertEqual(lib1_name, 'lib/abi1/libfoo.so')
      self.assertEqual(lib1_offset, 10)

      lib2_name, lib2_offset = libs_map.FindLibraryByOffset(1000)
      self.assertEqual(lib2_name, 'lib/abi2/libfoo.so')
      self.assertEqual(lib2_offset, 0)


class MockApkNativeLibraries(apk_native_libs.ApkNativeLibraries):
  """A mock ApkNativeLibraries instance that can be used as input to
     ApkLibraryPathTranslator without creating an ApkReader instance.

     Create a new instance, then call AddTestEntry or AddTestEntries
     as many times as necessary, before using it as a regular
     ApkNativeLibraries instance.
  """
  # pylint: disable=super-init-not-called
  def __init__(self):
    self._native_libs = []

  # pylint: enable=super-init-not-called

  def AddTestEntry(self, lib_path, file_offset, file_size):
    """Add a new test entry.

    Args:
      entry: A tuple of (library-path, file-offset, file-size) values,
          (e.g. ('lib/armeabi-v8a/libfoo.so', 0x10000, 0x2000)).
    """
    self._native_libs.append((lib_path, file_offset, file_offset + file_size))

  def AddTestEntries(self, entries):
    """Add a list of new test entries.

    Args:
      entries: A list of (library-path, file-offset, file-size) values.
    """
    for entry in entries:
      self.AddTestEntry(entry[0], entry[1], entry[2])


class MockApkNativeLibrariesTest(unittest.TestCase):

  def testEmptyInstance(self):
    mock = MockApkNativeLibraries()
    self.assertTrue(mock.IsEmpty())
    self.assertEqual(mock.GetLibraries(), [])
    self.assertEqual(mock.GetDumpList(), [])

  def testAddTestEntry(self):
    mock = MockApkNativeLibraries()
    mock.AddTestEntry('lib/armeabi-v7a/libfoo.so', 0x20000, 0x4000)
    mock.AddTestEntry('lib/x86/libzoo.so', 0x10000, 0x10000)
    mock.AddTestEntry('lib/armeabi-v7a/libbar.so', 0x24000, 0x8000)
    self.assertFalse(mock.IsEmpty())
    self.assertEqual(mock.GetLibraries(), ['lib/armeabi-v7a/libbar.so',
                                           'lib/armeabi-v7a/libfoo.so',
                                           'lib/x86/libzoo.so'])
    self.assertEqual(mock.GetDumpList(), [
        ('lib/x86/libzoo.so', 0x10000, 0x10000),
        ('lib/armeabi-v7a/libfoo.so', 0x20000, 0x4000),
        ('lib/armeabi-v7a/libbar.so', 0x24000, 0x8000),
    ])

  def testAddTestEntries(self):
    mock = MockApkNativeLibraries()
    mock.AddTestEntries([
      ('lib/armeabi-v7a/libfoo.so', 0x20000, 0x4000),
      ('lib/x86/libzoo.so', 0x10000, 0x10000),
      ('lib/armeabi-v7a/libbar.so', 0x24000, 0x8000),
    ])
    self.assertFalse(mock.IsEmpty())
    self.assertEqual(mock.GetLibraries(), ['lib/armeabi-v7a/libbar.so',
                                           'lib/armeabi-v7a/libfoo.so',
                                           'lib/x86/libzoo.so'])
    self.assertEqual(mock.GetDumpList(), [
        ('lib/x86/libzoo.so', 0x10000, 0x10000),
        ('lib/armeabi-v7a/libfoo.so', 0x20000, 0x4000),
        ('lib/armeabi-v7a/libbar.so', 0x24000, 0x8000),
    ])


class ApkLibraryPathTranslatorTest(unittest.TestCase):

  def _CheckUntranslated(self, translator, path, offset):
    """Check that a given (path, offset) is not modified by translation."""
    self.assertEqual(translator.TranslatePath(path, offset), (path, offset))


  def _CheckTranslated(self, translator, path, offset, new_path, new_offset):
    """Check that (path, offset) is translated into (new_path, new_offset)."""
    self.assertEqual(translator.TranslatePath(path, offset),
                     (new_path, new_offset))

  def testEmptyInstance(self):
    translator = apk_native_libs.ApkLibraryPathTranslator()
    self._CheckUntranslated(
        translator, '/data/data/com.example.app-1/base.apk', 0x123456)

  def testSimpleApk(self):
    mock_libs = MockApkNativeLibraries()
    mock_libs.AddTestEntries([
      ('lib/test-abi/libfoo.so', 200, 2000),
      ('lib/test-abi/libbar.so', 3200, 3000),
      ('lib/test-abi/crazy.libzoo.so', 6200, 2000),
    ])
    translator = apk_native_libs.ApkLibraryPathTranslator()
    translator.AddHostApk('com.example.app', mock_libs)

    # Offset is within the first uncompressed library
    self._CheckTranslated(
        translator,
        '/data/data/com.example.app-9.apk', 757,
        '/data/data/com.example.app-9.apk!lib/libfoo.so', 557)

    # Offset is within the second compressed library.
    self._CheckUntranslated(
        translator,
        '/data/data/com.example.app-9/base.apk', 2800)

    # Offset is within the third uncompressed library.
    self._CheckTranslated(
        translator,
        '/data/data/com.example.app-1/base.apk', 3628,
        '/data/data/com.example.app-1/base.apk!lib/libbar.so', 428)

    # Offset is within the fourth uncompressed library with crazy. prefix
    self._CheckTranslated(
        translator,
        '/data/data/com.example.app-XX/base.apk', 6500,
        '/data/data/com.example.app-XX/base.apk!lib/libzoo.so', 300)

    # Out-of-bounds apk offset.
    self._CheckUntranslated(
        translator,
        '/data/data/com.example.app-1/base.apk', 10000)

    # Invalid package name.
    self._CheckUntranslated(
        translator, '/data/data/com.example2.app-1/base.apk', 757)

    # Invalid apk name.
    self._CheckUntranslated(
          translator, '/data/data/com.example.app-2/not-base.apk', 100)

    # Invalid file extensions.
    self._CheckUntranslated(
          translator, '/data/data/com.example.app-2/base', 100)

    self._CheckUntranslated(
          translator, '/data/data/com.example.app-2/base.apk.dex', 100)

  def testBundleApks(self):
    mock_libs1 = MockApkNativeLibraries()
    mock_libs1.AddTestEntries([
      ('lib/test-abi/libfoo.so', 200, 2000),
      ('lib/test-abi/libbbar.so', 3200, 3000),
    ])
    mock_libs2 = MockApkNativeLibraries()
    mock_libs2.AddTestEntries([
      ('lib/test-abi/libzoo.so', 200, 2000),
      ('lib/test-abi/libtool.so', 3000, 4000),
    ])
    translator = apk_native_libs.ApkLibraryPathTranslator()
    translator.AddHostApk('com.example.app', mock_libs1, 'base-master.apk')
    translator.AddHostApk('com.example.app', mock_libs2, 'feature-master.apk')

    self._CheckTranslated(
      translator,
      '/data/app/com.example.app-XUIYIUW/base-master.apk', 757,
      '/data/app/com.example.app-XUIYIUW/base-master.apk!lib/libfoo.so', 557)

    self._CheckTranslated(
      translator,
      '/data/app/com.example.app-XUIYIUW/feature-master.apk', 3200,
      '/data/app/com.example.app-XUIYIUW/feature-master.apk!lib/libtool.so',
      200)


if __name__ == '__main__':
  unittest.main()
