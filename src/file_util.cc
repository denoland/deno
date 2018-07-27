// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include <inttypes.h>
#include <stdio.h>
#include <fstream>
#include <iterator>
#include <string>

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

}  // namespace deno
