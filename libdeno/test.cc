// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "test.h"
#include "file_util.h"

deno_buf snapshot = {nullptr, 0, nullptr, 0};

int main(int argc, char** argv) {
  // Load the snapshot.
  std::string contents;
  if (!deno::ReadFileToString(SNAPSHOT_PATH, &contents)) {
    printf("Failed to read file %s\n", SNAPSHOT_PATH);
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
