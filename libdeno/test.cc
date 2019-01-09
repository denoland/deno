// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "test.h"
#include "file_util.h"

deno_buf snapshot = {nullptr, 0, nullptr, 0};

int main(int argc, char** argv) {
  // Locate the snapshot.
  std::string exe_path;
  if (!deno::ExePath(&exe_path)) {
    std::cerr << "deno::ExePath() failed" << std::endl;
    return 1;
  }
  std::string snapshot_path = deno::Dirname(exe_path) + SNAPSHOT_PATH;

  // Load the snapshot.
  std::string contents;
  if (!deno::ReadFileToString(snapshot_path.c_str(), &contents)) {
    std::cerr << "Failed to read snapshot from " << snapshot_path << std::endl;
    return 1;
  }
  snapshot.data_ptr =
      reinterpret_cast<uint8_t*>(const_cast<char*>(contents.c_str()));
  snapshot.data_len = contents.size();

  testing::InitGoogleTest(&argc, argv);
  deno_init();
  deno_set_v8_flags(&argc, argv);
  return RUN_ALL_TESTS();
}
