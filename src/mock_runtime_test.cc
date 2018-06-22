// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include "testing/gtest/include/gtest/gtest.h"

#include "include/deno.h"

TEST(MockRuntimeTest, InitializesCorrectly) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "1 + 2"));
  deno_delete(d);
}

TEST(MockRuntimeTest, CanCallFunction) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js",
                           "if (CanCallFunction() != 'foo') throw Error();"));
  deno_delete(d);
}

TEST(MockRuntimeTest, ErrorsCorrectly) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_FALSE(deno_execute(d, "a.js", "throw Error()"));
  deno_delete(d);
}

deno_buf strbuf(const char* str) { return deno_buf{str, strlen(str)}; }

TEST(MockRuntimeTest, PubSuccess) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "PubSuccess()"));
  EXPECT_TRUE(deno_pub(d, "PubSuccess", strbuf("abc")));
  deno_delete(d);
}

TEST(MockRuntimeTest, PubByteLength) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "PubByteLength()"));
  // We pub the wrong sized message, it should throw.
  EXPECT_FALSE(deno_pub(d, "PubByteLength", strbuf("abcd")));
  deno_delete(d);
}

TEST(MockRuntimeTest, PubNoCallback) {
  Deno* d = deno_new(nullptr, nullptr);
  // We didn't call deno_sub(), pubing should fail.
  EXPECT_FALSE(deno_pub(d, "PubNoCallback", strbuf("abc")));
  deno_delete(d);
}

TEST(MockRuntimeTest, SubReturnEmpty) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto _, auto channel, auto buf) {
    count++;
    EXPECT_STREQ(channel, "SubReturnEmpty");
    EXPECT_EQ(static_cast<size_t>(3), buf.len);
    EXPECT_EQ(buf.data[0], 'a');
    EXPECT_EQ(buf.data[1], 'b');
    EXPECT_EQ(buf.data[2], 'c');
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "SubReturnEmpty()"));
  EXPECT_EQ(count, 2);
  deno_delete(d);
}

TEST(MockRuntimeTest, SubReturnBar) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto deno, auto channel, auto buf) {
    count++;
    EXPECT_STREQ(channel, "SubReturnBar");
    EXPECT_EQ(static_cast<size_t>(3), buf.len);
    EXPECT_EQ(buf.data[0], 'a');
    EXPECT_EQ(buf.data[1], 'b');
    EXPECT_EQ(buf.data[2], 'c');
    deno_set_response(deno, strbuf("bar"));
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "SubReturnBar()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(MockRuntimeTest, DoubleSubFails) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_FALSE(deno_execute(d, "a.js", "DoubleSubFails()"));
  deno_delete(d);
}

TEST(MockRuntimeTest, TypedArraySnapshots) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "TypedArraySnapshots()"));
  deno_delete(d);
}

TEST(MockRuntimeTest, SnapshotBug) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SnapshotBug()"));
  deno_delete(d);
}

TEST(MockRuntimeTest, ErrorHandling) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto deno, auto channel, auto buf) {
    count++;
    EXPECT_STREQ(channel, "ErrorHandling");
    EXPECT_EQ(static_cast<size_t>(1), buf.len);
    EXPECT_EQ(buf.data[0], 42);
  });
  EXPECT_FALSE(deno_execute(d, "a.js", "ErrorHandling()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

int main(int argc, char** argv) {
  testing::InitGoogleTest(&argc, argv);
  deno_init();
  deno_set_flags(&argc, argv);
  return RUN_ALL_TESTS();
}
