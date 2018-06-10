// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include "testing/gtest/include/gtest/gtest.h"

#include "include/deno.h"

TEST(DenoTest, InitializesCorrectly) {
  deno_init();
  Deno* d = deno_new(NULL, NULL);
  int r = deno_load(d, "a.js", "1 + 2");
  EXPECT_EQ(r, 0);
}

int main(int argc, char** argv) {
  testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
