// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include <inttypes.h>
#include <stdio.h>
#include <fstream>
#include <iterator>
#include <string>

#include "file_util.h"

namespace deno {

std::string BinaryContentAsC(const char* name, const std::string& data) {
  char b[512];
  std::string output;
  // Write prefix.
  snprintf(b, sizeof(b), "static const char %s_data[] = {\n", name);
  output.append(b);
  // Write actual data.
  for (size_t i = 0; i < data.size(); ++i) {
    if ((i & 0x1F) == 0x1F) output.append("\n");
    if (i > 0) output.append(",");
    snprintf(b, sizeof(b), "%hhu", static_cast<unsigned char>(data.at(i)));
    output.append(b);
  }
  output.append("\n");
  // Write suffix.
  output.append("};\n");
  snprintf(b, sizeof(b), "static const int %s_size = %" PRId64 ";\n", name,
           static_cast<uint64_t>(data.size()));
  output.append(b);
  return output;
}

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
