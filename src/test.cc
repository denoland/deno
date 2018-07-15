// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include "deno.h"
#include "testing/gtest/include/gtest/gtest.h"

int main(int argc, char** argv) {
  testing::InitGoogleTest(&argc, argv);
  deno_init();
  deno_set_flags(&argc, argv);
  return RUN_ALL_TESTS();
}
