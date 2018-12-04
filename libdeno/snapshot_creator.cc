// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Hint: --trace_serializer is a useful debugging flag.
#include <fstream>
#include "deno.h"
#include "file_util.h"
#include "internal.h"
#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

namespace deno {}  // namespace deno

int main(int argc, char** argv) {
  const char* snapshot_out_bin = argv[1];
  const char* js_fn = argv[2];
  const char* source_map_fn = argv[3];  // Optional.

  v8::V8::SetFlagsFromCommandLine(&argc, argv, true);

  CHECK_NE(js_fn, nullptr);
  CHECK_NE(snapshot_out_bin, nullptr);

  std::string js_source;
  CHECK(deno::ReadFileToString(js_fn, &js_source));

  std::string source_map;
  if (source_map_fn != nullptr) {
    CHECK_EQ(argc, 4);
    CHECK(deno::ReadFileToString(source_map_fn, &source_map));
  }

  deno_init();
  deno_config config = {deno::empty_buf, nullptr};
  Deno* d = deno_new_snapshotter(
      config, js_fn, js_source.c_str(),
      source_map_fn != nullptr ? source_map.c_str() : nullptr);

  auto snapshot = deno_get_snapshot(d);

  std::ofstream file_(snapshot_out_bin, std::ios::binary);
  file_.write(reinterpret_cast<char*>(snapshot.data_ptr), snapshot.data_len);
  file_.close();

  delete[] snapshot.data_ptr;
  deno_delete(d);

  return file_.bad();
}
