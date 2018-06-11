// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include "testing/gtest/include/gtest/gtest.h"

#include "include/deno.h"

TEST(MockRuntimeTest, InitializesCorrectly) {
  Deno* d = deno_new(NULL, NULL);
  EXPECT_TRUE(deno_load(d, "a.js", "1 + 2"));
  deno_dispose(d);
}

TEST(MockRuntimeTest, CanCallFoo) {
  Deno* d = deno_new(NULL, NULL);
  EXPECT_TRUE(deno_load(d, "a.js", "if (foo() != 'foo') throw Error();"));
  deno_dispose(d);
}

TEST(MockRuntimeTest, ErrorsCorrectly) {
  Deno* d = deno_new(NULL, NULL);
  EXPECT_FALSE(deno_load(d, "a.js", "throw Error()"));
  deno_dispose(d);
}

deno_buf strbuf(const char* str) {
  void* d = reinterpret_cast<void*>(const_cast<char*>(str));
  return deno_buf{d, strlen(str)};
}

TEST(MockRuntimeTest, PubSuccess) {
  Deno* d = deno_new(NULL, NULL);
  EXPECT_TRUE(deno_load(d, "a.js", "subabc();"));
  EXPECT_TRUE(deno_pub(d, strbuf("abc")));
  deno_dispose(d);
}

TEST(MockRuntimeTest, PubByteLength) {
  Deno* d = deno_new(NULL, NULL);
  EXPECT_TRUE(deno_load(d, "a.js", "subabc();"));
  // We pub the wrong sized message, it should throw.
  EXPECT_FALSE(deno_pub(d, strbuf("abcd")));
  deno_dispose(d);
}

TEST(MockRuntimeTest, PubNoCallback) {
  Deno* d = deno_new(NULL, NULL);
  // We didn't call deno_sub(), pubing should fail.
  EXPECT_FALSE(deno_pub(d, strbuf("abc")));
  deno_dispose(d);
}

int main(int argc, char** argv) {
  testing::InitGoogleTest(&argc, argv);
  deno_init();
  return RUN_ALL_TESTS();
}
