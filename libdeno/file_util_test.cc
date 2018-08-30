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
  EXPECT_EQ("", deno::Basename("/foo/"));
  EXPECT_EQ("", deno::Basename("foo/"));
  EXPECT_EQ("", deno::Basename("/"));
  EXPECT_EQ("foo.txt", deno::Basename(".\\foo.txt"));
  EXPECT_EQ("foo.txt", deno::Basename("/home/ryan/foo.txt"));
  EXPECT_EQ("foo.txt", deno::Basename("C:\\home\\ryan\\foo.txt"));
}

TEST(FileUtilTest, Dirname) {
  EXPECT_EQ("home/dank/", deno::Dirname("home/dank/memes.gif"));
  EXPECT_EQ("/home/dank/", deno::Dirname("/home/dank/memes.gif"));
  EXPECT_EQ("/home/dank/", deno::Dirname("/home/dank/"));
  EXPECT_EQ("home/dank/", deno::Dirname("home/dank/memes.gif"));
  EXPECT_EQ("/", deno::Dirname("/"));
  EXPECT_EQ(".\\", deno::Dirname(".\\memes.gif"));
  EXPECT_EQ("c:\\", deno::Dirname("c:\\stuff"));
  EXPECT_EQ("./", deno::Dirname("nothing"));
  EXPECT_EQ("./", deno::Dirname(""));
}

TEST(FileUtilTest, ExePath) {
  std::string exe_path;
  EXPECT_TRUE(deno::ExePath(&exe_path));
  // Path is absolute.
  EXPECT_TRUE(exe_path.find("/") == 0 || exe_path.find(":\\") == 1);
  // FIlename is the name of the test binary.
  std::string exe_filename = deno::Basename(exe_path);
  EXPECT_EQ(exe_filename.find("test_cc"), 0u);
  // Path exists (also tests ReadFileToString).
  std::string contents;
  EXPECT_TRUE(deno::ReadFileToString(exe_path.c_str(), &contents));
  EXPECT_NE(contents.find("Inception :)"), std::string::npos);
}
