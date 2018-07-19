// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
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

TEST(FileUtilTest, BinaryContentAsC) {
  auto c_code = deno::BinaryContentAsC("aaa", std::string("bbb"));
  EXPECT_TRUE(c_code.find("static const char aaa_data[]") != std::string::npos);
  EXPECT_TRUE(c_code.find("static const int aaa_size = 3;") !=
              std::string::npos);
}

// TODO(ry) success unit test. Needs a tempfile or fixture.
// TEST(FileUtilTest, ReadFileToStringSuccess) { }
