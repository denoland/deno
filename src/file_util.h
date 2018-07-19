// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#ifndef FILE_UTIL_H_
#define FILE_UTIL_H_

#include <string>

namespace deno {
bool ReadFileToString(const char* fn, std::string* contents);
std::string Basename(std::string const& filename);
std::string BinaryContentAsC(const char* name, const std::string& data);
}  // namespace deno

#endif  // FILE_UTIL_H_
