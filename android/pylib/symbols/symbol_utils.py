# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import bisect
import collections
import logging
import os
import re

from pylib.constants import host_paths
from pylib.symbols import elf_symbolizer


def _AndroidAbiToCpuArch(android_abi):
  """Return the Chromium CPU architecture name for a given Android ABI."""
  _ARCH_MAP = {
    'armeabi': 'arm',
    'armeabi-v7a': 'arm',
    'arm64-v8a': 'arm64',
    'x86_64': 'x64',
  }
  return _ARCH_MAP.get(android_abi, android_abi)


def _HexAddressRegexpFor(android_abi):
  """Return a regexp matching hexadecimal addresses for a given Android ABI."""
  if android_abi in ['x86_64', 'arm64-v8a', 'mips64']:
    width = 16
  else:
    width = 8
  return '[0-9a-f]{%d}' % width


class HostLibraryFinder(object):
  """Translate device library path to matching host unstripped library path.

  Usage is the following:
    1) Create instance.
    2) Call AddSearchDir() once or more times to add host directory path to
       look for unstripped native libraries.
    3) Call Find(device_libpath) repeatedly to translate a device-specific
       library path into the corresponding host path to the unstripped
       version.
  """
  def __init__(self):
    """Initialize instance."""
    self._search_dirs = []
    self._lib_map = {}        # Map of library name to host file paths.

  def AddSearchDir(self, lib_dir):
    """Add a directory to the search path for host native shared libraries.

    Args:
      lib_dir: host path containing native libraries.
    """
    if not os.path.exists(lib_dir):
      logging.warning('Ignoring missing host library directory: %s', lib_dir)
      return
    if not os.path.isdir(lib_dir):
      logging.warning('Ignoring invalid host library directory: %s', lib_dir)
      return
    self._search_dirs.append(lib_dir)
    self._lib_map = {}  # Reset the map.

  def Find(self, device_libpath):
    """Find the host file path matching a specific device library path.

    Args:
      device_libpath: device-specific file path to library or executable.
    Returns:
      host file path to the unstripped version of the library, or None.
    """
    host_lib_path = None
    lib_name = os.path.basename(device_libpath)
    host_lib_path = self._lib_map.get(lib_name)
    if not host_lib_path:
      for search_dir in self._search_dirs:
        lib_path = os.path.join(search_dir, lib_name)
        if os.path.exists(lib_path):
          host_lib_path = lib_path
          break

      if not host_lib_path:
        logging.debug('Could not find host library for: %s', lib_name)
      self._lib_map[lib_name] = host_lib_path

    return host_lib_path



class SymbolResolver(object):
  """A base class for objets that can symbolize library (path, offset)
     pairs into symbol information strings. Usage is the following:

     1) Create new instance (by calling the constructor of a derived
        class, since this is only the base one).

     2) Call SetAndroidAbi() before any call to FindSymbolInfo() in order
        to set the Android CPU ABI used for symbolization.

     3) Before the first call to FindSymbolInfo(), one can call
        AddLibraryOffset(), or AddLibraryOffsets() to record a set of offsets
        that you will want to symbolize later through FindSymbolInfo(). Doing
        so allows some SymbolResolver derived classes to work faster (e.g. the
        one that invokes the 'addr2line' program, since the latter works faster
        if the offsets provided as inputs are sorted in increasing order).

     3) Call FindSymbolInfo(path, offset) to return the corresponding
        symbol information string, or None if this doesn't correspond
        to anything the instance can handle.

        Note that whether the path is specific to the device or to the
        host depends on the derived class implementation.
  """
  def __init__(self):
    self._android_abi = None
    self._lib_offsets_map = collections.defaultdict(set)

  def SetAndroidAbi(self, android_abi):
    """Set the Android ABI value for this instance.

    Calling this function before FindSymbolInfo() is required by some
    derived class implementations.

    Args:
      android_abi: Native Android CPU ABI name (e.g. 'armeabi-v7a').
    Raises:
      Exception if the ABI was already set with a different value.
    """
    if self._android_abi and self._android_abi != android_abi:
      raise Exception('Cannot reset Android ABI to new value %s, already set '
                      'to %s' % (android_abi, self._android_abi))

    self._android_abi = android_abi

  def AddLibraryOffset(self, lib_path, offset):
    """Associate a single offset to a given device library.

    This must be called before FindSymbolInfo(), otherwise its input arguments
    will be ignored.

    Args:
      lib_path: A library path.
      offset: An integer offset within the corresponding library that will be
        symbolized by future calls to FindSymbolInfo.
    """
    self._lib_offsets_map[lib_path].add(offset)

  def AddLibraryOffsets(self, lib_path, lib_offsets):
    """Associate a set of wanted offsets to a given device library.

    This must be called before FindSymbolInfo(), otherwise its input arguments
    will be ignored.

    Args:
      lib_path: A library path.
      lib_offsets: An iterable of integer offsets within the corresponding
        library that will be symbolized by future calls to FindSymbolInfo.
    """
    self._lib_offsets_map[lib_path].update(lib_offsets)

  # pylint: disable=unused-argument,no-self-use
  def FindSymbolInfo(self, lib_path, lib_offset):
    """Symbolize a device library path and offset.

    Args:
      lib_path: Library path (device or host specific, depending on the
        derived class implementation).
      lib_offset: Integer offset within the library.
    Returns:
      Corresponding symbol information string, or None.
    """
    # The base implementation cannot symbolize anything.
    return None
  # pylint: enable=unused-argument,no-self-use


class ElfSymbolResolver(SymbolResolver):
  """A SymbolResolver that can symbolize host path + offset values using
     an elf_symbolizer.ELFSymbolizer instance.
  """
  def __init__(self, addr2line_path_for_tests=None):
    super(ElfSymbolResolver, self).__init__()
    self._addr2line_path = addr2line_path_for_tests

    # Used to cache one ELFSymbolizer instance per library path.
    self._elf_symbolizer_cache = {}

    # Used to cache FindSymbolInfo() results. Maps host library paths
    # to (offset -> symbol info string) dictionaries.
    self._symbol_info_cache = collections.defaultdict(dict)
    self._allow_symbolizer = True

  def _CreateSymbolizerFor(self, host_path):
    """Create the ELFSymbolizer instance associated with a given lib path."""
    addr2line_path = self._addr2line_path
    if not addr2line_path:
      if not self._android_abi:
        raise Exception(
            'Android CPU ABI must be set before calling FindSymbolInfo!')

      cpu_arch = _AndroidAbiToCpuArch(self._android_abi)
      self._addr2line_path = host_paths.ToolPath('addr2line', cpu_arch)

    return elf_symbolizer.ELFSymbolizer(
        elf_file_path=host_path, addr2line_path=self._addr2line_path,
        callback=ElfSymbolResolver._Callback, inlines=True)

  def DisallowSymbolizerForTesting(self):
    """Disallow FindSymbolInfo() from using a symbolizer.

    This is used during unit-testing to ensure that the offsets that were
    recorded via AddLibraryOffset()/AddLibraryOffsets() are properly
    symbolized, but not anything else.
    """
    self._allow_symbolizer = False

  def FindSymbolInfo(self, host_path, offset):
    """Override SymbolResolver.FindSymbolInfo.

    Args:
      host_path: Host-specific path to the native shared library.
      offset: Integer offset within the native library.
    Returns:
      A symbol info string, or None.
    """
    offset_map = self._symbol_info_cache[host_path]
    symbol_info = offset_map.get(offset)
    if symbol_info:
      return symbol_info

    # Create symbolizer on demand.
    symbolizer = self._elf_symbolizer_cache.get(host_path)
    if not symbolizer:
      symbolizer = self._CreateSymbolizerFor(host_path)
      self._elf_symbolizer_cache[host_path] = symbolizer

      # If there are pre-recorded offsets for this path, symbolize them now.
      offsets = self._lib_offsets_map.get(host_path)
      if offsets:
        offset_map = {}
        for pre_offset in offsets:
          symbolizer.SymbolizeAsync(
              pre_offset, callback_arg=(offset_map, pre_offset))
        symbolizer.WaitForIdle()
        self._symbol_info_cache[host_path] = offset_map

        symbol_info = offset_map.get(offset)
        if symbol_info:
          return symbol_info

    if not self._allow_symbolizer:
      return None

    # Symbolize single offset. Slower if addresses are not provided in
    # increasing order to addr2line.
    symbolizer.SymbolizeAsync(offset,
                              callback_arg=(offset_map, offset))
    symbolizer.WaitForIdle()
    return offset_map.get(offset)

  @staticmethod
  def _Callback(sym_info, callback_arg):
    offset_map, offset = callback_arg
    offset_map[offset] = str(sym_info)


class DeviceSymbolResolver(SymbolResolver):
  """A SymbolResolver instance that accepts device-specific path.

  Usage is the following:
    1) Create new instance, passing a parent SymbolResolver instance that
       accepts host-specific paths, and a HostLibraryFinder instance.

    2) Optional: call AddApkOffsets() to add offsets from within an APK
       that contains uncompressed native shared libraries.

    3) Use it as any SymbolResolver instance.
  """
  def __init__(self, host_resolver, host_lib_finder):
    """Initialize instance.

    Args:
      host_resolver: A parent SymbolResolver instance that will be used
        to resolve symbols from host library paths.
      host_lib_finder: A HostLibraryFinder instance used to locate
        unstripped libraries on the host.
    """
    super(DeviceSymbolResolver, self).__init__()
    self._host_lib_finder = host_lib_finder
    self._bad_device_lib_paths = set()
    self._host_resolver = host_resolver

  def SetAndroidAbi(self, android_abi):
    super(DeviceSymbolResolver, self).SetAndroidAbi(android_abi)
    self._host_resolver.SetAndroidAbi(android_abi)

  def AddLibraryOffsets(self, device_lib_path, lib_offsets):
    """Associate a set of wanted offsets to a given device library.

    This must be called before FindSymbolInfo(), otherwise its input arguments
    will be ignored.

    Args:
      device_lib_path: A device-specific library path.
      lib_offsets: An iterable of integer offsets within the corresponding
        library that will be symbolized by future calls to FindSymbolInfo.
        want to symbolize.
    """
    if device_lib_path in self._bad_device_lib_paths:
      return

    host_lib_path = self._host_lib_finder.Find(device_lib_path)
    if not host_lib_path:
      # NOTE: self._bad_device_lib_paths is only used to only print this
      #       warning once per bad library.
      logging.warning('Could not find host library matching device path: %s',
                      device_lib_path)
      self._bad_device_lib_paths.add(device_lib_path)
      return

    self._host_resolver.AddLibraryOffsets(host_lib_path, lib_offsets)

  def AddApkOffsets(self, device_apk_path, apk_offsets, apk_translator):
    """Associate a set of wanted offsets to a given device APK path.

    This converts the APK-relative offsets into offsets relative to the
    uncompressed libraries it contains, then calls AddLibraryOffsets()
    for each one of the libraries.

    Must be called before FindSymbolInfo() as well, otherwise input arguments
    will be ignored.

    Args:
      device_apk_path: Device-specific APK path.
      apk_offsets: Iterable of offsets within the APK file.
      apk_translator: An ApkLibraryPathTranslator instance used to extract
        library paths from the APK.
    """
    libraries_map = collections.defaultdict(set)
    for offset in apk_offsets:
      lib_path, lib_offset = apk_translator.TranslatePath(device_apk_path,
                                                          offset)
      libraries_map[lib_path].add(lib_offset)

    for lib_path, lib_offsets in libraries_map.iteritems():
      self.AddLibraryOffsets(lib_path, lib_offsets)

  def FindSymbolInfo(self, device_path, offset):
    """Overrides SymbolResolver.FindSymbolInfo.

    Args:
      device_path: Device-specific library path (e.g.
        '/data/app/com.example.app-1/lib/x86/libfoo.so')
      offset: Offset in device library path.
    Returns:
      Corresponding symbol information string, or None.
    """
    host_path = self._host_lib_finder.Find(device_path)
    if not host_path:
      return None

    return self._host_resolver.FindSymbolInfo(host_path, offset)


class MemoryMap(object):
  """Models the memory map of a given process. Usage is:

    1) Create new instance, passing the Android ABI.

    2) Call TranslateLine() whenever you want to detect and translate any
       memory map input line.

    3) Otherwise, it is possible to parse the whole memory map input with
       ParseLines(), then call FindSectionForAddress() repeatedly in order
       to translate a memory address into the corresponding mapping and
       file information tuple (e.g. to symbolize stack entries).
  """

  # A named tuple describing interesting memory map line items.
  # Fields:
  #   addr_start: Mapping start address in memory.
  #   file_offset: Corresponding file offset.
  #   file_size: Corresponding mapping size in bytes.
  #   file_path: Input file path.
  #   match: Corresponding regular expression match object.
  LineTuple = collections.namedtuple('MemoryMapLineTuple',
                                     'addr_start,file_offset,file_size,'
                                     'file_path, match')

  # A name tuple describing a memory map section.
  # Fields:
  #   address: Memory address.
  #   size: Size in bytes in memory
  #   offset: Starting file offset.
  #   path: Input file path.
  SectionTuple = collections.namedtuple('MemoryMapSection',
                                        'address,size,offset,path')

  def __init__(self, android_abi):
    """Initializes instance.

    Args:
      android_abi: Android CPU ABI name (e.g. 'armeabi-v7a')
    """
    hex_addr = _HexAddressRegexpFor(android_abi)

    # pylint: disable=line-too-long
    # A regular expression used to match memory map entries which look like:
    #    b278c000-b2790fff r--   4fda000      5000  /data/app/com.google.android.apps.chrome-2/base.apk
    # pylint: enable=line-too-long
    self._re_map_section = re.compile(
        r'\s*(?P<addr_start>' + hex_addr + r')-(?P<addr_end>' + hex_addr + ')' +
        r'\s+' +
        r'(?P<perm>...)\s+' +
        r'(?P<file_offset>[0-9a-f]+)\s+' +
        r'(?P<file_size>[0-9a-f]+)\s*' +
        r'(?P<file_path>[^ \t]+)?')

    self._addr_map = []  # Sorted list of (address, size, path, offset) tuples.
    self._sorted_addresses = []  # Sorted list of address fields in _addr_map.
    self._in_section = False

  def TranslateLine(self, line, apk_path_translator):
    """Try to translate a memory map input line, if detected.

    This only takes care of converting mapped APK file path and offsets
    into a corresponding uncompressed native library file path + new offsets,
    e.g. '..... <offset> <size> /data/.../base.apk' gets
    translated into '.... <new-offset> <size> /data/.../base.apk!lib/libfoo.so'

    This function should always work, even if ParseLines() was not called
    previously.

    Args:
      line: Input memory map / tombstone line.
      apk_translator: An ApkLibraryPathTranslator instance, used to map
        APK offsets into uncompressed native libraries + new offsets.
    Returns:
      Translated memory map line, if relevant, or unchanged input line
      otherwise.
    """
    t = self._ParseLine(line.rstrip())
    if not t:
      return line

    new_path, new_offset = apk_path_translator.TranslatePath(
        t.file_path, t.file_offset)

    if new_path == t.file_path:
      return line

    pos = t.match.start('file_path')
    return '%s%s (offset 0x%x)%s' % (line[0:pos], new_path, new_offset,
                                     line[t.match.end('file_path'):])

  def ParseLines(self, input_lines, in_section=False):
    """Parse a list of input lines and extract the APK memory map out of it.

    Args:
      input_lines: list, or iterable, of input lines.
      in_section: Optional. If true, considers that the input lines are
        already part of the memory map. Otherwise, wait until the start of
        the section appears in the input before trying to record data.
    Returns:
      True iff APK-related memory map entries were found. False otherwise.
    """
    addr_list = []  # list of (address, size, file_path, file_offset) tuples.
    self._in_section = in_section
    for line in input_lines:
      t = self._ParseLine(line.rstrip())
      if not t:
        continue

      addr_list.append(t)

    self._addr_map = sorted(addr_list,
                            lambda x, y: cmp(x.addr_start, y.addr_start))
    self._sorted_addresses = [e.addr_start for e in self._addr_map]
    return bool(self._addr_map)

  def _ParseLine(self, line):
    """Used internally to recognized memory map input lines.

    Args:
      line: Input logcat or tomstone line.
    Returns:
      A LineTuple instance on success, or None on failure.
    """
    if not self._in_section:
      self._in_section = line.startswith('memory map:')
      return None

    m = self._re_map_section.match(line)
    if not m:
      self._in_section = False  # End of memory map section
      return None

    # Only accept .apk and .so files that are not from the system partitions.
    file_path = m.group('file_path')
    if not file_path:
      return None

    if file_path.startswith('/system') or file_path.startswith('/vendor'):
      return None

    if not (file_path.endswith('.apk') or file_path.endswith('.so')):
      return None

    addr_start = int(m.group('addr_start'), 16)
    file_offset = int(m.group('file_offset'), 16)
    file_size = int(m.group('file_size'), 16)

    return self.LineTuple(addr_start, file_offset, file_size, file_path, m)

  def Dump(self):
    """Print memory map for debugging."""
    print 'MEMORY MAP ['
    for t in self._addr_map:
      print '[%08x-%08x %08x %08x %s]' % (
          t.addr_start, t.addr_start + t.file_size, t.file_size, t.file_offset,
          t.file_path)
    print '] MEMORY MAP'

  def FindSectionForAddress(self, addr):
    """Find the map section corresponding to a specific memory address.

    Call this method only after using ParseLines() was called to extract
    relevant information from the memory map.

    Args:
      addr: Memory address
    Returns:
      A SectionTuple instance on success, or None on failure.
    """
    pos = bisect.bisect_right(self._sorted_addresses, addr)
    if pos > 0:
      # All values in [0,pos) are <= addr, just ensure that the last
      # one contains the address as well.
      entry = self._addr_map[pos - 1]
      if entry.addr_start + entry.file_size > addr:
        return self.SectionTuple(entry.addr_start, entry.file_size,
                                 entry.file_offset, entry.file_path)
    return None


class BacktraceTranslator(object):
  """Translates backtrace-related lines in a tombstone or crash report.

  Usage is the following:
    1) Create new instance with appropriate arguments.
    2) If the tombstone / logcat input is available, one can call
       FindLibraryOffsets() in order to detect which library offsets
       will need to be symbolized during a future parse. Doing so helps
       speed up the ELF symbolizer.
    3) For each tombstone/logcat input line, call TranslateLine() to
       try to detect and symbolize backtrace lines.
  """

  # A named tuple for relevant input backtrace lines.
  # Fields:
  #   rel_pc: Instruction pointer, relative to offset in library start.
  #   location: Library or APK file path.
  #   offset: Load base of executable code in library or apk file path.
  #   match: The corresponding regular expression match object.
  # Note:
  #   The actual instruction pointer always matches the position at
  #   |offset + rel_pc| in |location|.
  LineTuple = collections.namedtuple('BacktraceLineTuple',
                                      'rel_pc,location,offset,match')

  def __init__(self, android_abi, apk_translator):
    """Initialize instance.

    Args:
      android_abi: Android CPU ABI name (e.g. 'armeabi-v7a').
      apk_translator: ApkLibraryPathTranslator instance used to convert
        mapped APK file offsets into uncompressed library file paths with
        new offsets.
    """
    hex_addr = _HexAddressRegexpFor(android_abi)

    # A regular expression used to match backtrace lines.
    self._re_backtrace = re.compile(
        r'.*#(?P<frame>[0-9]{2})\s+' +
        r'(..)\s+' +
        r'(?P<rel_pc>' + hex_addr + r')\s+' +
        r'(?P<location>[^ \t]+)' +
        r'(\s+\(offset 0x(?P<offset>[0-9a-f]+)\))?')

    # In certain cases, offset will be provided as <location>+0x<offset>
    # instead of <location> (offset 0x<offset>). This is a regexp to detect
    # this.
    self._re_location_offset = re.compile(
        r'.*\+0x(?P<offset>[0-9a-f]+)$')

    self._apk_translator = apk_translator
    self._in_section = False

  def _ParseLine(self, line):
    """Used internally to detect and decompose backtrace input lines.

    Args:
      line: input tombstone line.
    Returns:
      A LineTuple instance on success, None on failure.
    """
    if not self._in_section:
      self._in_section = line.startswith('backtrace:')
      return None

    line = line.rstrip()
    m = self._re_backtrace.match(line)
    if not m:
      self._in_section = False
      return None

    location = m.group('location')
    offset = m.group('offset')
    if not offset:
      m2 = self._re_location_offset.match(location)
      if m2:
        offset = m2.group('offset')
        location = location[0:m2.start('offset') - 3]

    if not offset:
      return None

    offset = int(offset, 16)
    rel_pc = int(m.group('rel_pc'), 16)

    # Two cases to consider here:
    #
    # * If this is a library file directly mapped in memory, then |rel_pc|
    #   if the direct offset within the library, and doesn't need any kind
    #   of adjustement.
    #
    # * If this is a library mapped directly from an .apk file, then
    #   |rel_pc| is the offset in the APK, and |offset| happens to be the
    #   load base of the corresponding library.
    #
    if location.endswith('.so'):
      # For a native library directly mapped from the file system,
      return self.LineTuple(rel_pc, location, offset, m)

    if location.endswith('.apk'):
      # For a native library inside an memory-mapped APK file,
      new_location, new_offset = self._apk_translator.TranslatePath(
          location, offset)

      return self.LineTuple(rel_pc, new_location, new_offset, m)

    # Ignore anything else (e.g. .oat or .odex files).
    return None

  def FindLibraryOffsets(self, input_lines, in_section=False):
    """Parse a tombstone's backtrace section and find all library offsets in it.

    Args:
      input_lines: List or iterables of intput tombstone lines.
      in_section: Optional. If True, considers that the stack section has
        already started.
    Returns:
      A dictionary mapping device library paths to sets of offsets within
      then.
    """
    self._in_section = in_section
    result = collections.defaultdict(set)
    for line in input_lines:
      t = self._ParseLine(line)
      if not t:
        continue

      result[t.location].add(t.offset + t.rel_pc)
    return result

  def TranslateLine(self, line, symbol_resolver):
    """Symbolize backtrace line if recognized.

    Args:
      line: input backtrace line.
      symbol_resolver: symbol resolver instance to use. This method will
        call its FindSymbolInfo(device_lib_path, lib_offset) method to
        convert offsets into symbol informations strings.
    Returns:
      Translated line (unchanged if not recognized as a back trace).
    """
    t = self._ParseLine(line)
    if not t:
      return line

    symbol_info = symbol_resolver.FindSymbolInfo(t.location,
                                                 t.offset + t.rel_pc)
    if not symbol_info:
      symbol_info = 'offset 0x%x' % t.offset

    pos = t.match.start('location')
    pos2 = t.match.end('offset') + 1
    if pos2 <= 0:
      pos2 = t.match.end('location')
    return '%s%s (%s)%s' % (line[:pos], t.location, symbol_info, line[pos2:])


class StackTranslator(object):
  """Translates stack-related lines in a tombstone or crash report."""

  # A named tuple describing relevant stack input lines.
  # Fields:
  #  address: Address as it appears in the stack.
  #  lib_path: Library path where |address| is mapped.
  #  lib_offset: Library load base offset. for |lib_path|.
  #  match: Corresponding regular expression match object.
  LineTuple = collections.namedtuple('StackLineTuple',
                                     'address, lib_path, lib_offset, match')

  def __init__(self, android_abi, memory_map, apk_translator):
    """Initialize instance."""
    hex_addr = _HexAddressRegexpFor(android_abi)

    # pylint: disable=line-too-long
    # A regular expression used to recognize stack entries like:
    #
    #    #05  bf89a180  bf89a1e4  [stack]
    #         bf89a1c8  a0c01c51  /data/app/com.google.android.apps.chrome-2/base.apk
    #         bf89a080  00000000
    #         ........  ........
    # pylint: enable=line-too-long
    self._re_stack_line = re.compile(
        r'\s+(?P<frame_number>#[0-9]+)?\s*' +
        r'(?P<stack_addr>' + hex_addr + r')\s+' +
        r'(?P<stack_value>' + hex_addr + r')' +
        r'(\s+(?P<location>[^ \t]+))?')

    self._re_stack_abbrev = re.compile(r'\s+[.]+\s+[.]+')

    self._memory_map = memory_map
    self._apk_translator = apk_translator
    self._in_section = False

  def _ParseLine(self, line):
    """Check a given input line for a relevant _re_stack_line match.

    Args:
      line: input tombstone line.
    Returns:
      A LineTuple instance on success, None on failure.
    """
    line = line.rstrip()
    if not self._in_section:
      self._in_section = line.startswith('stack:')
      return None

    m = self._re_stack_line.match(line)
    if not m:
      if not self._re_stack_abbrev.match(line):
        self._in_section = False
      return None

    location = m.group('location')
    if not location:
      return None

    if not location.endswith('.apk') and not location.endswith('.so'):
      return None

    addr = int(m.group('stack_value'), 16)
    t = self._memory_map.FindSectionForAddress(addr)
    if t is None:
      return None

    lib_path = t.path
    lib_offset = t.offset + (addr - t.address)

    if lib_path.endswith('.apk'):
      lib_path, lib_offset = self._apk_translator.TranslatePath(
          lib_path, lib_offset)

    return self.LineTuple(addr, lib_path, lib_offset, m)

  def FindLibraryOffsets(self, input_lines, in_section=False):
    """Parse a tombstone's stack section and find all library offsets in it.

    Args:
      input_lines: List or iterables of intput tombstone lines.
      in_section: Optional. If True, considers that the stack section has
        already started.
    Returns:
      A dictionary mapping device library paths to sets of offsets within
      then.
    """
    result = collections.defaultdict(set)
    self._in_section = in_section
    for line in input_lines:
      t = self._ParseLine(line)
      if t:
        result[t.lib_path].add(t.lib_offset)
    return result

  def TranslateLine(self, line, symbol_resolver=None):
    """Try to translate a line of the stack dump."""
    t = self._ParseLine(line)
    if not t:
      return line

    symbol_info = symbol_resolver.FindSymbolInfo(t.lib_path, t.lib_offset)
    if not symbol_info:
      return line

    pos = t.match.start('location')
    pos2 = t.match.end('location')
    return '%s%s (%s)%s' % (line[:pos], t.lib_path, symbol_info, line[pos2:])
