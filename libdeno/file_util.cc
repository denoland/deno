// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include <inttypes.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <fstream>
#include <iterator>
#include <string>

#ifdef __APPLE__
#include <mach-o/dyld.h>
#endif

#ifdef _WIN32
#include <windows.h>
#endif

#include "file_util.h"

namespace deno {

bool ReadFileToString(const char* fn, std::string* contents) {
  std::ifstream file(fn, std::ios::binary);
  if (file.fail()) {
    return false;
  }
  contents->assign(std::istreambuf_iterator<char>{file}, {});
  return !file.fail();
}

std::string Basename(std::string const& filename) {
  for (auto it = filename.rbegin(); it != filename.rend(); ++it) {
    char ch = *it;
    if (ch == '\\' || ch == '/') {
      return std::string(it.base(), filename.end());
    }
  }
  return filename;
}

// Returns the directory component from a filename. The returned path always
// ends with a slash. This function does not understand Windows drive letters.
std::string Dirname(std::string const& filename) {
  for (auto it = filename.rbegin(); it != filename.rend(); ++it) {
    char ch = *it;
    if (ch == '\\' || ch == '/') {
      return std::string(filename.begin(), it.base());
    }
  }
  return std::string("./");
}

// Returns the full path the currently running executable.
// This implementation is very basic. Caveats:
//   * OS X: fails if buffer is too small, does not retry with a bigger buffer.
//   * Windows: ANSI only, no unicode. Fails if path is longer than 260 chars.
bool ExePath(std::string* path) {
#ifdef _WIN32
  // Windows only.
  char exe_buf[MAX_PATH];
  DWORD len = GetModuleFileNameA(NULL, exe_buf, sizeof exe_buf);
  if (len == 0 || len == sizeof exe_buf) {
    return false;
  }
#else
#ifdef __APPLE__
  // OS X only.
  char link_buf[PATH_MAX * 2];  // Exe may be longer than MAX_PATH, says Apple.
  uint32_t len = sizeof link_buf;
  if (_NSGetExecutablePath(link_buf, &len) < 0) {
    return false;
  }
#else
  // Linux only.
  static const char* link_buf = "/proc/self/exe";
#endif
  // Linux and OS X.
  char exe_buf[PATH_MAX];
  char* r = realpath(link_buf, exe_buf);
  if (r == NULL) {
    return false;
  }
#endif
  // All platforms.
  path->assign(exe_buf);
  return true;
}

}  // namespace deno
