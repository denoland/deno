// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#include "test.h"
#include <fstream>
#include <string>
#include "internal.h"

deno_snapshot snapshot = {nullptr, 0};

bool ReadFileToString(const char* fn, std::string* contents) {
  std::ifstream file(fn, std::ios::binary);
  if (file.fail()) {
    return false;
  }
  contents->assign(std::istreambuf_iterator<char>{file}, {});
  return !file.fail();
}

int main(int argc, char** argv) {
  // All of the JS code in libdeno_test.js is tested after being snapshotted.
  // We create that snapshot now at runtime, rather than at compile time to
  // simplify the build process. So we load and execute the libdeno_test.js
  // file, without running any of the tests and store the result in the global
  // "snapshot" variable, which will be used later in the tests.
  std::string js_fn = JS_PATH;
  std::string js_source;
  CHECK(ReadFileToString(js_fn.c_str(), &js_source));

  deno_init();
  deno_config config = {1, deno::empty_snapshot, deno::empty_buf, nullptr,
                        nullptr};
  Deno* d = deno_new(config);

  deno_execute(d, nullptr, js_fn.c_str(), js_source.c_str());
  if (deno_last_exception(d) != nullptr) {
    std::cerr << "Snapshot Exception " << std::endl;
    std::cerr << deno_last_exception(d) << std::endl;
    deno_delete(d);
    return 1;
  }

  snapshot = deno_snapshot_new(d);

  testing::InitGoogleTest(&argc, argv);
  deno_init();
  deno_set_v8_flags(&argc, argv);
  return RUN_ALL_TESTS();
}
