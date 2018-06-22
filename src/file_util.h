// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#ifndef FILE_UTIL_H_
#define FILE_UTIL_H_

#include <string>

namespace deno {
bool WriteDataAsCpp(const char* name, const char* filename,
                    const std::string& data);
bool ReadFileToString(const char* fn, std::string* contents);
}  // namespace deno

#endif  // FILE_UTIL_H_
