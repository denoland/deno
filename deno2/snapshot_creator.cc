// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
// Hint: --trace_serializer is a useful debugging flag.
#include "deno_internal.h"
#include "file_util.h"
#include "include/deno.h"
#include "v8/include/v8.h"
#include "v8/src/base/logging.h"

v8::StartupData StringToStartupData(const std::string& s) {
  return v8::StartupData{s.c_str(), s.size()};
}

int main(int argc, char** argv) {
  const char* js_fn = argv[1];
  const char* natives_in_bin = argv[2];
  const char* snapshot_in_bin = argv[3];
  const char* natives_out_cc = argv[4];
  const char* snapshot_out_cc = argv[5];

  CHECK_NE(js_fn, nullptr);
  CHECK_NE(natives_in_bin, nullptr);
  CHECK_NE(snapshot_in_bin, nullptr);
  CHECK_NE(natives_out_cc, nullptr);
  CHECK_NE(snapshot_out_cc, nullptr);

  v8::V8::SetFlagsFromCommandLine(&argc, argv, true);

  std::string js_source;
  CHECK(deno::ReadFileToString(js_fn, &js_source));

  std::string natives_str;
  CHECK(deno::ReadFileToString(natives_in_bin, &natives_str));
  auto natives_blob = StringToStartupData(natives_str);

  std::string snapshot_in_str;
  CHECK(deno::ReadFileToString(snapshot_in_bin, &snapshot_in_str));
  auto snapshot_in_blob = StringToStartupData(snapshot_in_str);

  deno_init();
  auto snapshot_blob = deno::MakeSnapshot(&natives_blob, &snapshot_in_blob,
                                          js_fn, js_source.c_str());
  std::string snapshot_str(snapshot_blob.data, snapshot_blob.raw_size);

  CHECK(deno::WriteDataAsCpp("natives", natives_out_cc, natives_str));
  CHECK(deno::WriteDataAsCpp("snapshot", snapshot_out_cc, snapshot_str));
}
