// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include "testing/gtest/include/gtest/gtest.h"

#include "deno.h"

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

deno_buf strbuf(const char* str) {
  auto len = strlen(str);

  deno_buf buf;
  buf.alloc_ptr = reinterpret_cast<uint8_t*>(strdup(str));
  buf.alloc_len = len + 1;
  buf.data_ptr = buf.alloc_ptr;
  buf.data_len = len;

  return buf;
}

TEST(MockRuntimeTest, SendSuccess) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SendSuccess()"));
  EXPECT_TRUE(deno_send(d, strbuf("abc")));
  deno_delete(d);
}

TEST(MockRuntimeTest, SendByteLength) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SendByteLength()"));
  // We pub the wrong sized message, it should throw.
  EXPECT_FALSE(deno_send(d, strbuf("abcd")));
  deno_delete(d);
}

TEST(MockRuntimeTest, SendNoCallback) {
  Deno* d = deno_new(nullptr, nullptr);
  // We didn't call deno.recv() in JS, should fail.
  EXPECT_FALSE(deno_send(d, strbuf("abc")));
  deno_delete(d);
}

TEST(MockRuntimeTest, RecvReturnEmpty) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto _, auto buf) {
    count++;
    EXPECT_EQ(static_cast<size_t>(3), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 'a');
    EXPECT_EQ(buf.data_ptr[1], 'b');
    EXPECT_EQ(buf.data_ptr[2], 'c');
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "RecvReturnEmpty()"));
  EXPECT_EQ(count, 2);
  deno_delete(d);
}

TEST(MockRuntimeTest, RecvReturnBar) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto deno, auto buf) {
    count++;
    EXPECT_EQ(static_cast<size_t>(3), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 'a');
    EXPECT_EQ(buf.data_ptr[1], 'b');
    EXPECT_EQ(buf.data_ptr[2], 'c');
    deno_set_response(deno, strbuf("bar"));
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "RecvReturnBar()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(MockRuntimeTest, DoubleRecvFails) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_FALSE(deno_execute(d, "a.js", "DoubleRecvFails()"));
  deno_delete(d);
}

TEST(MockRuntimeTest, SendRecvSlice) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto deno, auto buf) {
    static const size_t alloc_len = 1024;
    size_t i = count++;
    // Check the size and offset of the slice.
    size_t data_offset = buf.data_ptr - buf.alloc_ptr;
    EXPECT_EQ(data_offset, i * 11);
    EXPECT_EQ(buf.data_len, alloc_len - i * 30);
    EXPECT_EQ(buf.alloc_len, alloc_len);
    // Check values written by the JS side.
    EXPECT_EQ(buf.data_ptr[0], 100 + i);
    EXPECT_EQ(buf.data_ptr[buf.data_len - 1], 100 - i);
    // Make copy of the backing buffer -- this is currently necessary because
    // deno_set_response() takes ownership over the buffer, but we are not given
    // ownership of `buf` by our caller.
    uint8_t* alloc_ptr = reinterpret_cast<uint8_t*>(malloc(alloc_len));
    memcpy(alloc_ptr, buf.alloc_ptr, alloc_len);
    // Make a slice that is a bit shorter than the original.
    deno_buf buf2{alloc_ptr, alloc_len, alloc_ptr + data_offset,
                  buf.data_len - 19};
    // Place some values into the buffer for the JS side to verify.
    buf2.data_ptr[0] = 200 + i;
    buf2.data_ptr[buf2.data_len - 1] = 200 - i;
    // Send back.
    deno_set_response(deno, buf2);
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "SendRecvSlice()"));
  EXPECT_EQ(count, 5);
  deno_delete(d);
}

TEST(MockRuntimeTest, JSSendArrayBufferViewTypes) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto _, auto buf) {
    count++;
    size_t data_offset = buf.data_ptr - buf.alloc_ptr;
    EXPECT_EQ(data_offset, 2468u);
    EXPECT_EQ(buf.data_len, 1000u);
    EXPECT_EQ(buf.alloc_len, 4321u);
    EXPECT_EQ(buf.data_ptr[0], count);
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "JSSendArrayBufferViewTypes()"));
  EXPECT_EQ(count, 3);
  deno_delete(d);
}

TEST(MockRuntimeTest, JSSendNeutersBuffer) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto _, auto buf) {
    count++;
    EXPECT_EQ(buf.data_len, 1u);
    EXPECT_EQ(buf.data_ptr[0], 42);
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "JSSendNeutersBuffer()"));
  EXPECT_EQ(count, 1);
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
  Deno* d = deno_new(nullptr, [](auto deno, auto buf) {
    count++;
    EXPECT_EQ(static_cast<size_t>(1), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 42);
  });
  EXPECT_FALSE(deno_execute(d, "a.js", "ErrorHandling()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}
