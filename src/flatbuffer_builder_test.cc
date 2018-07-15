// Copyright 2018 Bert Belder <bertbelder@gmail.com>
// All rights reserved. MIT License.

#include <stdint.h>

#include "testing/gtest/include/gtest/gtest.h"

#include "deno.h"
#include "flatbuffer_builder.h"

template <typename T, std::size_t N>
constexpr std::size_t countof(T const (&)[N]) noexcept {
  return N;
}

TEST(FlatBufferBuilderTest, ExportBuf) {
  const uint32_t nums[] = {1, 2, 3};
  const char str[] = "hello mars";
  deno_buf nums_buf;
  deno_buf str_buf;
  // Use scope so builder gets destroyed after building buffers.
  {
    deno::FlatBufferBuilder builder;
    // Build first flatbuffer.
    auto nums_fb = builder.CreateVector(nums, countof(nums));
    builder.Finish(nums_fb);
    nums_buf = builder.ExportBuf();
    // Reset builder.
    builder.Reset();
    // Build second flatbuffer using the same builder.
    auto str_fb = builder.CreateString(str);
    builder.Finish(str_fb);
    str_buf = builder.ExportBuf();
  }
  // Allocations should be different.
  EXPECT_NE(nums_buf.alloc_ptr, str_buf.alloc_ptr);
  // Logical buffer data should be contained inside their allocations.
  EXPECT_GE(nums_buf.data_ptr, nums_buf.alloc_ptr);
  EXPECT_LE(nums_buf.data_ptr + nums_buf.data_len,
            nums_buf.alloc_ptr + nums_buf.alloc_len);
  EXPECT_GE(str_buf.data_ptr, str_buf.alloc_ptr);
  EXPECT_LE(str_buf.data_ptr + str_buf.data_len,
            str_buf.alloc_ptr + str_buf.alloc_len);
  // Since there is no way to parse these buffers without generating code,
  // just check whether the data is contained in the raw content.
  // Both the numbers vector and the string start at offset 8 in the flatbuffer.
  auto nums_data = reinterpret_cast<uint32_t*>(nums_buf.data_ptr + 8);
  for (size_t i = 0; i < countof(nums); i++) {
    EXPECT_EQ(nums_data[i], nums[i]);
  }
  auto str_data = str_buf.data_ptr + 8;
  for (size_t i = 0; i < countof(str); i++) {
    EXPECT_EQ(str_data[i], str[i]);
  }
}

TEST(FlatBufferBuilderTest, CanGrowBuffer) {
  static const size_t kSmallInitialSize = 32;
  static const char zeroes[1024] = {0};
  {
    // Create buffer with small initial size.
    deno::FlatBufferBuilder builder(kSmallInitialSize);
    // Write 1 byte and export buffer.
    builder.Finish(builder.CreateVector(zeroes, 1));
    auto buf = builder.ExportBuf();
    // Exported buffer should have initial size.
    EXPECT_EQ(buf.alloc_len, kSmallInitialSize);
  }
  {
    // Create buffer with small initial size.
    deno::FlatBufferBuilder builder(kSmallInitialSize);
    // Write 1024 bytes and export buffer.
    builder.Finish(builder.CreateVector(zeroes, countof(zeroes)));
    auto buf = builder.ExportBuf();
    // Exported buffer have grown.
    EXPECT_GT(buf.alloc_len, kSmallInitialSize);
  }
}
