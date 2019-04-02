// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Hint: --trace_serializer is a useful debugging flag.
#include <fstream>
#include <iostream>
#include "deno.h"
#include "file_util.h"
#include "internal.h"
#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

int main(int argc, char** argv) {
  const char* snapshot_out_bin = argv[1];
  const char* js_fn = argv[2];

  deno_set_v8_flags(&argc, argv);

  CHECK_NOT_NULL(js_fn);
  CHECK_NOT_NULL(snapshot_out_bin);

  std::string js_source;
  CHECK(deno::ReadFileToString(js_fn, &js_source));

  deno_init();
  deno_config config = {1, deno::empty_snapshot, deno::empty_buf, nullptr};
  Deno* d = deno_new(config);

  deno_execute(d, nullptr, js_fn, js_source.c_str());
  if (deno_last_exception(d) != nullptr) {
    std::cerr << "Snapshot Exception " << std::endl;
    std::cerr << deno_last_exception(d) << std::endl;
    deno_delete(d);
    return 1;
  }

  auto snapshot = deno_get_snapshot(d);

  std::ofstream file_(snapshot_out_bin, std::ios::binary);
  file_.write(reinterpret_cast<char*>(snapshot.data_ptr), snapshot.data_len);
  file_.close();

  deno_snapshot_delete(snapshot);
  deno_delete(d);

  return file_.bad();
}
