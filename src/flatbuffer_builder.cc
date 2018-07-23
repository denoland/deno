// Copyright 2018 the Deno authors. All rights reserved. MIT license.

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include "deno.h"
#include "flatbuffer_builder.h"
#include "flatbuffers/flatbuffers.h"

namespace deno {

deno_buf FlatBufferBuilder::ExportBuf() {
  uint8_t* data_ptr = GetBufferPointer();
  size_t data_len = GetSize();
  return allocator_.GetAndKeepBuf(data_ptr, data_len);
}

deno_buf FlatBufferBuilder::Allocator::GetAndKeepBuf(uint8_t* data_ptr,
                                                     size_t data_len) {
  // The builder will typically allocate one chunk of memory with some
  // default size. After that, it'll only call allocate() again if the
  // initial allocation wasn't big enough, which is then immediately
  // followed by deallocate() to release the buffer that was too small.
  //
  // Therefore we can assume that the `data_ptr` points somewhere inside
  // the last allocation, and that we never have to protect earlier
  // allocations from being released.
  //
  // Each builder gets it's own Allocator instance, so multiple builders
  // can be exist at the same time without conflicts.

  assert(last_alloc_ptr_ != nullptr);   // Must have allocated.
  assert(keep_alloc_ptr_ == nullptr);   // Didn't export any buffer so far.
  assert(data_ptr >= last_alloc_ptr_);  // Data must be within allocation.
  assert(data_ptr + data_len <= last_alloc_ptr_ + last_alloc_len_);

  keep_alloc_ptr_ = last_alloc_ptr_;

  deno_buf buf;
  buf.alloc_ptr = last_alloc_ptr_;
  buf.alloc_len = last_alloc_len_;
  buf.data_ptr = data_ptr;
  buf.data_len = data_len;
  return buf;
}

uint8_t* FlatBufferBuilder::Allocator::allocate(size_t size) {
  auto ptr = reinterpret_cast<uint8_t*>(malloc(size));
  if (ptr == nullptr) {
    return nullptr;
  }

  last_alloc_ptr_ = ptr;
  last_alloc_len_ = size;

  return ptr;
}

void FlatBufferBuilder::Allocator::deallocate(uint8_t* ptr, size_t size) {
  if (ptr == last_alloc_ptr_) {
    last_alloc_ptr_ = nullptr;
    last_alloc_len_ = 0;
  }

  if (ptr == keep_alloc_ptr_) {
    // This allocation became an exported buffer, so don't free it.
    // Clearing keep_alloc_ptr_ makes it possible to export another
    // buffer later (after the builder is reset with `Reset()`).
    keep_alloc_ptr_ = nullptr;
    return;
  }

  free(ptr);
}

}  // namespace deno
