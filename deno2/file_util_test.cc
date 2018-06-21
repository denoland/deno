// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include "testing/gtest/include/gtest/gtest.h"

#include "file_util.h"

TEST(FileUtilTest, ReadFileToStringFileNotExist) {
  std::string output;
  EXPECT_FALSE(deno::ReadFileToString("/should_error_out.txt", &output));
}

// TODO(ry) success unit test. Needs a tempfile or fixture.
// TEST(FileUtilTest, ReadFileToStringSuccess) { }
