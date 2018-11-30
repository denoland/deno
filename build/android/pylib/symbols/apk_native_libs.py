# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging
import os
import re
import struct
import zipfile

# The default zipfile python module cannot open APKs properly, but this
# fixes it. Note that simply importing this file is sufficient to
# ensure that zip works correctly for all other modules. See:
# http://bugs.python.org/issue14315
# https://hg.python.org/cpython/rev/6dd5e9556a60#l2.8
def _PatchZipFile():
  # pylint: disable=protected-access
  oldDecodeExtra = zipfile.ZipInfo._decodeExtra
  def decodeExtra(self):
    try:
      oldDecodeExtra(self)
    except struct.error:
      pass
  zipfile.ZipInfo._decodeExtra = decodeExtra
_PatchZipFile()


class ApkZipInfo(object):
  """Models a single file entry from an ApkReader.

  This is very similar to the zipfile.ZipInfo class. It provides a few
  properties describing the entry:
    - filename          (same as ZipInfo.filename)
    - file_size         (same as ZipInfo.file_size)
    - compress_size     (same as ZipInfo.file_size)
    - file_offset       (note: not provided by ZipInfo)

  And a few useful methods: IsCompressed() and IsElfFile().

  Entries can be created by using ApkReader() methods.
  """
  def __init__(self, zip_file, zip_info):
    """Construct instance. Do not call this directly. Use ApkReader methods."""
    self._file = zip_file
    self._info = zip_info
    self._file_offset = None

  @property
  def filename(self):
    """Entry's file path within APK."""
    return self._info.filename

  @property
  def file_size(self):
    """Entry's extracted file size in bytes."""
    return self._info.file_size

  @property
  def compress_size(self):
    """Entry' s compressed file size in bytes."""
    return self._info.compress_size

  @property
  def file_offset(self):
    """Entry's starting file offset in the APK."""
    if self._file_offset is None:
      self._file_offset = self._ZipFileOffsetFromLocalHeader(
          self._file.fp, self._info.header_offset)
    return self._file_offset

  def __repr__(self):
    """Convert to string for debugging."""
    return 'ApkZipInfo["%s",size=0x%x,compressed=0x%x,offset=0x%x]' % (
        self.filename, self.file_size, self.compress_size, self.file_offset)

  def IsCompressed(self):
    """Returns True iff the entry is compressed."""
    return self._info.compress_type != zipfile.ZIP_STORED

  def IsElfFile(self):
    """Returns True iff the entry is an ELF file."""
    with self._file.open(self._info, 'r') as f:
      return f.read(4) == '\x7fELF'

  @staticmethod
  def _ZipFileOffsetFromLocalHeader(fd, local_header_offset):
    """Return a file's start offset from its zip archive local header.

    Args:
      fd: Input file object.
      local_header_offset: Local header offset (from its ZipInfo entry).
    Returns:
      file start offset.
    """
    FILE_NAME_LEN_OFFSET = 26
    FILE_NAME_OFFSET = 30
    fd.seek(local_header_offset + FILE_NAME_LEN_OFFSET)
    file_name_len = struct.unpack('H', fd.read(2))[0]
    extra_field_len = struct.unpack('H', fd.read(2))[0]
    file_offset = (local_header_offset + FILE_NAME_OFFSET +
                    file_name_len + extra_field_len)
    return file_offset


class ApkReader(object):
  """A convenience class used to read the content of APK files.

  Its design is very similar to the one from zipfile.ZipFile, except
  that its returns ApkZipInfo entries which provide a |file_offset|
  property that can be used to know where a given file is located inside
  the archive.

  It is also easy to mock for unit-testing (see MockApkReader in
  apk_utils_unittest.py) without creating any files on disk.

  Usage is the following:
    - Create an instance using a with statement (for proper unit-testing).
    - Call ListEntries() to list all entries in the archive. This returns
      a list of ApkZipInfo entries.
    - Or call FindEntry() corresponding to a given path within the archive.

  For example:
     with ApkReader(input_apk_path) as reader:
       info = reader.FindEntry('lib/armeabi-v7a/libfoo.so')
       if info.IsCompressed() or not info.IsElfFile():
         raise Exception('Invalid library path")

  The ApkZipInfo can be used to inspect the entry's metadata, or read its
  content with the ReadAll() method. See its documentation for all details.
  """
  def __init__(self, apk_path):
    """Initialize instance."""
    self._zip_file = zipfile.ZipFile(apk_path, 'r')
    self._path = apk_path

  def __enter__(self):
    """Python context manager entry."""
    return self

  def __exit__(self, *kwargs):
    """Python context manager exit."""
    self.Close()

  @property
  def path(self):
    """The corresponding input APK path."""
    return self._path

  def Close(self):
    """Close the reader (and underlying ZipFile instance)."""
    self._zip_file.close()

  def ListEntries(self):
    """Return a list of ApkZipInfo entries for this APK."""
    result = []
    for info in self._zip_file.infolist():
      result.append(ApkZipInfo(self._zip_file, info))
    return result

  def FindEntry(self, file_path):
    """Return an ApkZipInfo instance for a given archive file path.

    Args:
      file_path: zip file path.
    Return:
      A new ApkZipInfo entry on success.
    Raises:
      KeyError on failure (entry not found).
    """
    info = self._zip_file.getinfo(file_path)
    return ApkZipInfo(self._zip_file, info)



class ApkNativeLibraries(object):
  """A class for the list of uncompressed shared libraries inside an APK.

  Create a new instance by passing the path to an input APK, then use
  the FindLibraryByOffset() method to find the native shared library path
  corresponding to a given file offset.

  GetAbiList() and GetLibrariesList() can also be used to inspect
  the state of the instance.
  """
  def __init__(self, apk_reader):
    """Initialize instance.

    Args:
      apk_reader: An ApkReader instance corresponding to the input APK.
    """
    self._native_libs = []
    for entry in apk_reader.ListEntries():
      # Chromium uses so-called 'placeholder' native shared libraries
      # that have a size of 0, and are only used to deal with bugs in
      # older Android system releases (they are never loaded and cannot
      # appear in stack traces). Ignore these here to avoid generating
      # confusing results.
      if entry.file_size == 0:
        continue

      # Only uncompressed libraries can appear in stack traces.
      if entry.IsCompressed():
        continue

      # Only consider files within lib/ and with a filename ending with .so
      # at the moment. NOTE: Do not require a 'lib' prefix, since that would
      # prevent finding the 'crazy.libXXX.so' libraries used by Chromium.
      if (not entry.filename.startswith('lib/') or
          not entry.filename.endswith('.so')):
        continue

      lib_path = entry.filename

      self._native_libs.append(
          (lib_path, entry.file_offset, entry.file_offset + entry.file_size))

  def IsEmpty(self):
    """Return true iff the list is empty."""
    return not bool(self._native_libs)

  def GetLibraries(self):
    """Return the list of all library paths in this instance."""
    return sorted([x[0] for x in self._native_libs])

  def GetDumpList(self):
    """Retrieve full library map.

    Returns:
      A list of (lib_path, file_offset, file_size) tuples, sorted
      in increasing |file_offset| values.
    """
    result = []
    for entry in self._native_libs:
      lib_path, file_start, file_end = entry
      result.append((lib_path, file_start, file_end - file_start))

    return sorted(result, lambda x, y: cmp(x[1], y[1]))

  def FindLibraryByOffset(self, file_offset):
    """Find the native library at a given file offset.

    Args:
      file_offset: File offset within the original APK.
    Returns:
      Returns a (lib_path, lib_offset) tuple on success, or (None, 0)
      on failure. Note that lib_path will omit the 'lib/$ABI/' prefix,
      lib_offset is the adjustment of file_offset within the library.
    """
    for lib_path, start_offset, end_offset in self._native_libs:
      if file_offset >= start_offset and file_offset < end_offset:
        return (lib_path, file_offset - start_offset)

    return (None, 0)


class ApkLibraryPathTranslator(object):
  """Translates APK file paths + byte offsets into library path + offset.

  The purpose of this class is to translate a native shared library path
  that points to an APK into a new device-specific path that points to a
  native shared library, as if it was installed there. E.g.:

     ('/data/data/com.example.app-1/base.apk', 0x123be00)

  would be translated into:

     ('/data/data/com.example.app-1/base.apk!lib/libfoo.so', 0x3be00)

  If the original APK (installed as base.apk) contains an uncompressed shared
  library under lib/armeabi-v7a/libfoo.so at offset 0x120000.

  Note that the virtual device path after the ! doesn't necessarily match
  the path inside the .apk. This doesn't really matter for the rest of
  the symbolization functions since only the file's base name can be used
  to find the corresponding file on the host.

  Usage is the following:

     1/ Create new instance.

     2/ Call AddHostApk() one or several times to add the host path
        of an APK, its package name, and device-installed named.

     3/ Call TranslatePath() to translate a (path, offset) tuple corresponding
        to an on-device APK, into the corresponding virtual device library
        path and offset.
  """

  # Depending on the version of the system, a non-system APK might be installed
  # on a path that looks like the following:
  #
  #  * /data/..../<package_name>-<number>.apk, where <number> is used to
  #    distinguish several versions of the APK during package updates.
  #
  #  * /data/..../<package_name>-<suffix>/base.apk, where <suffix> is a
  #    string of random ASCII characters following the dash after the
  #    package name. This serves as a way to distinguish the installation
  #    paths during package update, and randomize its final location
  #    (to prevent apps from hard-coding the paths to other apps).
  #
  #    Note that the 'base.apk' name comes from the system.
  #
  #  * /data/.../<package_name>-<suffix>/<split_name>.apk, where <suffix>
  #    is the same as above, and <split_name> is the name of am app bundle
  #    split APK.
  #
  # System APKs are installed on paths that look like /system/app/Foo.apk
  # but this class ignores them intentionally.

  # Compiler regular expression for the first format above.
  _RE_APK_PATH_1 = re.compile(
      r'/data/.*/(?P<package_name>[A-Za-z0-9_.]+)-(?P<version>[0-9]+)\.apk')

  # Compiled regular expression for the second and third formats above.
  _RE_APK_PATH_2 = re.compile(
      r'/data/.*/(?P<package_name>[A-Za-z0-9_.]+)-(?P<suffix>[^/]+)/' +
      r'(?P<apk_name>.+\.apk)')

  def __init__(self):
    """Initialize instance. Call AddHostApk() to add host apk file paths."""
    self._path_map = {}  # Maps (package_name, apk_name) to host-side APK path.
    self._libs_map = {}  # Maps APK host path to ApkNativeLibrariesMap instance.

  def AddHostApk(self, package_name, native_libs, device_apk_name=None):
    """Add a file path to the host APK search list.

    Args:
      package_name: Corresponding apk package name.
      native_libs: ApkNativeLibraries instance for the corresponding APK.
      device_apk_name: Optional expected name of the installed APK on the
        device. This is only useful when symbolizing app bundle that run on
        Android L+. I.e. it will be ignored in other cases.
    """
    if native_libs.IsEmpty():
      logging.debug('Ignoring host APK without any uncompressed native ' +
                    'libraries: %s', device_apk_name)
      return

    # If the APK name is not provided, use the default of 'base.apk'. This
    # will be ignored if we find <package_name>-<number>.apk file paths
    # in the input, but will work properly for Android L+, as long as we're
    # not using Android app bundles.
    device_apk_name = device_apk_name or 'base.apk'

    key = "%s/%s" % (package_name, device_apk_name)
    if key in self._libs_map:
      raise KeyError('There is already an APK associated with (%s)' % key)

    self._libs_map[key] = native_libs

  @staticmethod
  def _MatchApkDeviceInstallPath(apk_path):
    """Check whether a given path matches an installed APK device file path.

    Args:
      apk_path: Device-specific file path.
    Returns:
      On success, a (package_name, apk_name) tuple. On failure, (None. None).
    """
    m = ApkLibraryPathTranslator._RE_APK_PATH_1.match(apk_path)
    if m:
      return (m.group('package_name'), 'base.apk')

    m = ApkLibraryPathTranslator._RE_APK_PATH_2.match(apk_path)
    if m:
      return (m.group('package_name'), m.group('apk_name'))

    return (None, None)

  def TranslatePath(self, apk_path, apk_offset):
    """Translate a potential apk file path + offset into library path + offset.

    Args:
      apk_path: Library or apk file path on the device (e.g.
        '/data/data/com.example.app-XSAHKSJH/base.apk').
      apk_offset: Byte offset within the library or apk.

    Returns:
      a new (lib_path, lib_offset) tuple. If |apk_path| points to an APK,
      then this function searches inside the corresponding host-side APKs
      (added with AddHostApk() above) for the corresponding uncompressed
      native shared library at |apk_offset|, if found, this returns a new
      device-specific path corresponding to a virtual installation of said
      library with an adjusted offset.

      Otherwise, just return the original (apk_path, apk_offset) values.
    """
    if not apk_path.endswith('.apk'):
      return (apk_path, apk_offset)

    apk_package, apk_name = self._MatchApkDeviceInstallPath(apk_path)
    if not apk_package:
      return (apk_path, apk_offset)

    key = '%s/%s' % (apk_package, apk_name)
    native_libs = self._libs_map.get(key)
    if not native_libs:
      logging.debug('Unknown %s package', key)
      return (apk_path, apk_offset)

    lib_name, new_offset = native_libs.FindLibraryByOffset(apk_offset)
    if not lib_name:
      logging.debug('Invalid offset in %s.apk package: %d', key, apk_offset)
      return (apk_path, apk_offset)

    lib_name = os.path.basename(lib_name)

    # Some libraries are stored with a crazy. prefix inside the APK, this
    # is done to prevent the PackageManager from extracting the libraries
    # at installation time when running on pre Android M systems, where the
    # system linker cannot load libraries directly from APKs.
    crazy_prefix = 'crazy.'
    if lib_name.startswith(crazy_prefix):
      lib_name = lib_name[len(crazy_prefix):]

    # Put this in a fictional lib sub-directory for good measure.
    new_path = '%s!lib/%s' % (apk_path, lib_name)

    return (new_path, new_offset)
