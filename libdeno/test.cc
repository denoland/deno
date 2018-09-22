// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "deno.h"
#include "testing/gtest/include/gtest/gtest.h"

int main(int argc, char** argv) {
  testing::InitGoogleTest(&argc, argv);
  deno_init();
  deno_set_v8_flags(&argc, argv);
  return RUN_ALL_TESTS();
}
