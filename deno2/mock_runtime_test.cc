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

TEST(MockRuntimeTest, SubReturnEmpty) {
  static int count = 0;
  Deno* d = deno_new(NULL, [](Deno* _, deno_buf buf) {
    count++;
    EXPECT_EQ(static_cast<size_t>(3), buf.len);
    // TODO(ry) buf.data should just be a char*.
    char* data = reinterpret_cast<char*>(buf.data);
    EXPECT_EQ(data[0], 'a');
    EXPECT_EQ(data[1], 'b');
    EXPECT_EQ(data[2], 'c');
    return deno_buf{nullptr, 0};
  });
  EXPECT_TRUE(deno_load(d, "a.js", "pubReturnEmpty()"));
  EXPECT_EQ(count, 2);
  deno_dispose(d);
}

TEST(MockRuntimeTest, SubReturnBar) {
  static int count = 0;
  Deno* d = deno_new(NULL, [](Deno* _, deno_buf buf) {
    count++;
    EXPECT_EQ(static_cast<size_t>(3), buf.len);
    // TODO(ry) buf.data should just be a char*.
    char* data = reinterpret_cast<char*>(buf.data);
    EXPECT_EQ(data[0], 'a');
    EXPECT_EQ(data[1], 'b');
    EXPECT_EQ(data[2], 'c');
    return strbuf("bar");
  });
  EXPECT_TRUE(deno_load(d, "a.js", "pubReturnBar()"));
  EXPECT_EQ(count, 1);
  deno_dispose(d);
}

int main(int argc, char** argv) {
  testing::InitGoogleTest(&argc, argv);
  deno_init();
  return RUN_ALL_TESTS();
}
