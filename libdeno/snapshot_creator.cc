// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Hint: --trace_serializer is a useful debugging flag.
#include <fstream>
#include <iostream>
#include "deno.h"
#include "file_util.h"
#include "internal.h"
#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

namespace deno {}  // namespace deno

int main(int argc, char** argv) {
  const char* snapshot_out_bin = argv[1];
  const char* js_fn = argv[2];

  deno_set_v8_flags(&argc, argv);

  CHECK_NOT_NULL(js_fn);
  CHECK_NOT_NULL(snapshot_out_bin);

  std::string js_source;
  CHECK(deno::ReadFileToString(js_fn, &js_source));

  auto snapshot = deno_generate_snapshot(js_fn, js_source.c_str());

  std::ofstream file_(snapshot_out_bin, std::ios::binary);
  file_.write(reinterpret_cast<char*>(snapshot.data_ptr), snapshot.data_len);
  file_.close();

  delete[] snapshot.data_ptr;

  return file_.bad();
}
