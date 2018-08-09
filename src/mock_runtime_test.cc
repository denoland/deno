// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "testing/gtest/include/gtest/gtest.h"

#include "deno.h"

static uint32_t mock_cmd_id_cb(const deno_buf* buf) { return 0; }

static Deno* deno_new_mock(void* data, deno_recv_cb recv_cb,
                           deno_cmd_id_cb cmd_id_cb = mock_cmd_id_cb) {
  return deno_new(data, recv_cb, cmd_id_cb);
}

TEST(MockRuntimeTest, InitializesCorrectly) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "1 + 2"));
  deno_delete(d);
}

TEST(MockRuntimeTest, CanCallFunction) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js",
                           "if (CanCallFunction() != 'foo') throw Error();"));
  deno_delete(d);
}

TEST(MockRuntimeTest, ErrorsCorrectly) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  EXPECT_FALSE(deno_execute(d, "a.js", "throw Error()"));
  deno_delete(d);
}

const deno_buf strbuf(const char* str) {
  auto len = strlen(str);
  auto ptr = reinterpret_cast<uint8_t*>(strdup(str));
  return deno_buf_new_raw(ptr, len + 1, ptr, len);
}

// Same as strbuf but with null alloc_ptr.
const deno_buf StrBufNullAllocPtr(const char* str) {
  auto len = strlen(str);
  return deno_buf_new_raw(nullptr, 0, reinterpret_cast<uint8_t*>(strdup(str)),
                          len);
}

TEST(MockRuntimeTest, SendSuccess) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SendSuccess()"));
  EXPECT_TRUE(deno_send(d, strbuf("abc")));
  deno_delete(d);
}

TEST(MockRuntimeTest, SendWrongByteLength) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SendWrongByteLength()"));
  // deno_send the wrong sized message, it should throw.
  EXPECT_FALSE(deno_send(d, strbuf("abcd")));
  std::string exception = deno_last_exception(d);
  EXPECT_GT(exception.length(), 1u);
  EXPECT_NE(exception.find("assert"), std::string::npos);
  deno_delete(d);
}

TEST(MockRuntimeTest, SendNoCallback) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  // We didn't call deno.recv() in JS, should fail.
  EXPECT_FALSE(deno_send(d, strbuf("abc")));
  deno_delete(d);
}

// TEST(MockRuntimeTest, RecvReturnEmpty) {
//  static int count = 0;
//  Deno* d = deno_new_mock(nullptr, [](auto _, auto buf) {
//    count++;
//    EXPECT_EQ(static_cast<size_t>(3), buf->data_len);
//    EXPECT_EQ(buf->data_ptr[0], 'a');
//    EXPECT_EQ(buf->data_ptr[1], 'b');
//    EXPECT_EQ(buf->data_ptr[2], 'c');
//  });
//  EXPECT_TRUE(deno_execute(d, "a.js", "RecvReturnEmpty()"));
//  EXPECT_EQ(count, 2);
//  deno_delete(d);
//}

TEST(MockRuntimeTest, RecvReturnBar) {
  static int count = 0;
  Deno* d = deno_new_mock(nullptr, [](auto deno, auto buf) {
    count++;
    EXPECT_EQ(static_cast<size_t>(3), buf->data_len);
    EXPECT_EQ(buf->data_ptr[0], 'a');
    EXPECT_EQ(buf->data_ptr[1], 'b');
    EXPECT_EQ(buf->data_ptr[2], 'c');
    deno_set_response(deno, strbuf("bar"));
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "RecvReturnBar()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(MockRuntimeTest, DoubleRecvFails) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  EXPECT_FALSE(deno_execute(d, "a.js", "DoubleRecvFails()"));
  deno_delete(d);
}

TEST(MockRuntimeTest, SendRecvSlice) {
  static int count = 0;
  Deno* d = deno_new_mock(nullptr, [](auto deno, auto buf_) {
    static const size_t alloc_len = 1024;
    size_t i = count++;
    // Take ownership of the buffer that was passed in.
    auto buf = deno_buf_move(buf_);
    // Check the size and offset of the slice.
    size_t data_offset = buf.data_ptr - buf.alloc_ptr;
    EXPECT_EQ(data_offset, i * 11);
    EXPECT_EQ(buf.data_len, alloc_len - i * 30);
    EXPECT_EQ(buf.alloc_len, alloc_len);
    // Check values written by the JS side.
    EXPECT_EQ(buf.data_ptr[0], 100 + i);
    EXPECT_EQ(buf.data_ptr[buf.data_len - 1], 100 - i);
    // Make the slice somewhat shorter than it was.
    buf.data_len -= 19;
    // Place some values into the buffer for the JS side to verify.
    buf.data_ptr[0] = 200 + i;
    buf.data_ptr[buf.data_len - 1] = 200 - i;
    // Send back.
    deno_set_response(deno, deno_buf_move(&buf));
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "SendRecvSlice()"));
  EXPECT_EQ(count, 5);
  deno_delete(d);
}

TEST(MockRuntimeTest, JSSendArrayBufferViewTypes) {
  static int count = 0;
  Deno* d = deno_new_mock(nullptr, [](auto deno, auto buf) {
    count++;
    size_t data_offset = buf->data_ptr - buf->alloc_ptr;
    EXPECT_EQ(data_offset, 2468u);
    EXPECT_EQ(buf->data_len, 1000u);
    EXPECT_EQ(buf->alloc_len, 4321u);
    EXPECT_EQ(buf->data_ptr[0], count);
    deno_set_response(deno, DENO_BUF_NULL);
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "JSSendArrayBufferViewTypes()"));
  EXPECT_EQ(count, 3);
  deno_delete(d);
}

TEST(MockRuntimeTest, JSSendNeutersBuffer) {
  static int count = 0;
  Deno* d = deno_new_mock(nullptr, [](auto deno, auto buf) {
    count++;
    EXPECT_EQ(buf->data_len, 1u);
    EXPECT_EQ(buf->data_ptr[0], 42);
    // Send back.
    deno_set_response(deno, DENO_BUF_NULL);
  });
  EXPECT_TRUE(deno_execute(d, "a.js", "JSSendNeutersBuffer()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(MockRuntimeTest, TypedArraySnapshots) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "TypedArraySnapshots()"));
  deno_delete(d);
}

TEST(MockRuntimeTest, SnapshotBug) {
  Deno* d = deno_new_mock(nullptr, nullptr);
  EXPECT_TRUE(deno_execute(d, "a.js", "SnapshotBug()"));
  deno_delete(d);
}

TEST(MockRuntimeTest, ErrorHandling) {
  static int count = 0;
  Deno* d = deno_new_mock(nullptr, [](auto deno, auto buf) {
    count++;
    EXPECT_EQ(static_cast<size_t>(1), buf->data_len);
    EXPECT_EQ(buf->data_ptr[0], 42);
    deno_set_response(deno, DENO_BUF_NULL);
  });
  EXPECT_FALSE(deno_execute(d, "a.js", "ErrorHandling()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(MockRuntimeTest, SendNullAllocPtr) {
  static int count = 0;
  Deno* d = deno_new_mock(nullptr, [](auto _, auto buf) { count++; });
  EXPECT_TRUE(deno_execute(d, "a.js", "SendNullAllocPtr()"));
  auto res_buf = StrBufNullAllocPtr("abcd");
  EXPECT_EQ(res_buf.alloc_ptr, nullptr);
  EXPECT_EQ(res_buf.data_len, 4u);
  EXPECT_TRUE(deno_send(d, deno_buf_move(&res_buf)));
  EXPECT_EQ(count, 0);
  deno_delete(d);
}
