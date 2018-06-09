// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include "testing/gtest/include/gtest/gtest.h"

#include "./deno.h"

TEST(SnapshotTest, InitializesCorrectly) {
  EXPECT_TRUE(true);
  // TODO(ry) add actual tests
}

int main(int argc, char** argv) {
  testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
