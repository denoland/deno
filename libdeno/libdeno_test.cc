// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "testing/gtest/include/gtest/gtest.h"

#include "deno.h"

TEST(LibDenoTest, InitializesCorrectly) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "1 + 2"));
  deno_delete(d);
}

TEST(LibDenoTest, CanCallFunction) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js",
                           "if (CanCallFunction() != 'foo') throw Error();"));
  deno_delete(d);
}

TEST(LibDenoTest, ErrorsCorrectly) {
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

// Same as strbuf but with null alloc_ptr.
deno_buf StrBufNullAllocPtr(const char* str) {
  auto len = strlen(str);
  deno_buf buf;
  buf.alloc_ptr = nullptr;
  buf.alloc_len = 0;
  buf.data_ptr = reinterpret_cast<uint8_t*>(strdup(str));
  buf.data_len = len;
  return buf;
}

TEST(LibDenoTest, SendSuccess) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SendSuccess()"));
  EXPECT_TRUE(deno_send(d, strbuf("abc")));
  deno_delete(d);
}

TEST(LibDenoTest, SendWrongByteLength) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SendWrongByteLength()"));
  // deno_send the wrong sized message, it should throw.
  EXPECT_FALSE(deno_send(d, strbuf("abcd")));
  std::string exception = deno_last_exception(d);
  EXPECT_GT(exception.length(), 1u);
  EXPECT_NE(exception.find("assert"), std::string::npos);
  deno_delete(d);
}

TEST(LibDenoTest, SendNoCallback) {
  Deno* d = deno_new(nullptr, nullptr);
  // We didn't call deno.recv() in JS, should fail.
  EXPECT_FALSE(deno_send(d, strbuf("abc")));
  deno_delete(d);
}

TEST(LibDenoTest, RecvReturnEmpty) {
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

TEST(LibDenoTest, RecvReturnBar) {
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

TEST(LibDenoTest, DoubleRecvFails) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_FALSE(deno_execute(d, "a.js", "DoubleRecvFails()"));
  deno_delete(d);
}

TEST(LibDenoTest, SendRecvSlice) {
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

TEST(LibDenoTest, JSSendArrayBufferViewTypes) {
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

TEST(LibDenoTest, TypedArraySnapshots) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "TypedArraySnapshots()"));
  deno_delete(d);
}

TEST(LibDenoTest, SnapshotBug) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SnapshotBug()"));
  deno_delete(d);
}

TEST(LibDenoTest, GlobalErrorHandling) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto _, auto buf) {
    count++;
    EXPECT_EQ(static_cast<size_t>(1), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 42);
  });
  EXPECT_FALSE(deno_execute(d, "a.js", "GlobalErrorHandling()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(LibDenoTest, DoubleGlobalErrorHandlingFails) {
  Deno* d = deno_new(nullptr, nullptr);
  EXPECT_FALSE(deno_execute(d, "a.js", "DoubleGlobalErrorHandlingFails()"));
  deno_delete(d);
}

TEST(LibDenoTest, SendNullAllocPtr) {
  static int count = 0;
  Deno* d = deno_new(nullptr, [](auto _, auto buf) { count++; });
  EXPECT_TRUE(deno_execute(d, "a.js", "SendNullAllocPtr()"));
  deno_buf buf = StrBufNullAllocPtr("abcd");
  EXPECT_EQ(buf.alloc_ptr, nullptr);
  EXPECT_EQ(buf.data_len, 4u);
  EXPECT_TRUE(deno_send(d, buf));
  EXPECT_EQ(count, 0);
  deno_delete(d);
}
