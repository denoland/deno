// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "testing/gtest/include/gtest/gtest.h"

#include "file_util.h"

TEST(FileUtilTest, ReadFileToStringFileNotExist) {
  std::string output;
  EXPECT_FALSE(deno::ReadFileToString("/should_error_out.txt", &output));
}

TEST(FileUtilTest, Basename) {
  EXPECT_EQ("foo.txt", deno::Basename("foo.txt"));
  EXPECT_EQ("foo.txt", deno::Basename("/foo.txt"));
  EXPECT_EQ("", deno::Basename("/"));
  EXPECT_EQ("foo.txt", deno::Basename(".\\foo.txt"));
  EXPECT_EQ("foo.txt", deno::Basename("/home/ryan/foo.txt"));
  EXPECT_EQ("foo.txt", deno::Basename("C:\\home\\ryan\\foo.txt"));
}

// TODO(ry) success unit test. Needs a tempfile or fixture.
// TEST(FileUtilTest, ReadFileToStringSuccess) { }
