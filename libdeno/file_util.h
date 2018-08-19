// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef FILE_UTIL_H_
#define FILE_UTIL_H_

#include <string>

namespace deno {
bool ReadFileToString(const char* fn, std::string* contents);
std::string Basename(std::string const& filename);
}  // namespace deno

#endif  // FILE_UTIL_H_
