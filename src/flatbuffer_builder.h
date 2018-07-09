// Copyright 2018 Bert Belder <bertbelder@gmail.com>
// All rights reserved. MIT License.
#ifndef FLATBUFFER_BUILDER_H_
#define FLATBUFFER_BUILDER_H_

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include "deno.h"
#include "flatbuffers/flatbuffers.h"

namespace deno {

// Wrap the default FlatBufferBuilder class, because the default one can't give
// us a pointer to the output buffer that we own. Nominally,
// FlatBufferBuilder::Release() should do that, but it returns some
// smart-pointer-like object (DetachedBuffer) that frees the buffer when it goes
// out of scope.
//
// This wrapper adds the `ExportBuf` method that returns a deno_buf, which
// is really not owned at all -- the caller is responsible for releasing the
// allocation with free().
//
// The alternative allocator also uses malloc()/free(), rather than
// new/delete[], so that the exported buffer can be later be converted to an
// ArrayBuffer; the (default) V8 ArrayBuffer allocator also uses free().
class FlatBufferBuilder : public flatbuffers::FlatBufferBuilder {
  static const size_t kDefaultInitialSize = 1024;

  class Allocator : public flatbuffers::Allocator {
    uint8_t* keep_alloc_ptr_ = nullptr;
    uint8_t* last_alloc_ptr_ = nullptr;
    size_t last_alloc_len_ = 0;

   public:
    deno_buf GetAndKeepBuf(uint8_t* data_ptr, size_t data_len);

   protected:
    virtual uint8_t* allocate(size_t size);
    virtual void deallocate(uint8_t* ptr, size_t size);
  };

  Allocator allocator_;

 public:
  explicit FlatBufferBuilder(size_t initial_size = kDefaultInitialSize)
      : flatbuffers::FlatBufferBuilder(initial_size, &allocator_) {}

  // Export the finalized flatbuffer as a deno_buf structure. The caller takes
  // ownership of the underlying memory allocation, which must be released with
  // free().
  // Afer calling ExportBuf() the FlatBufferBuilder should no longer be used;
  // However it can be used again once it is reset with the Reset() method.
  deno_buf ExportBuf();

  // Don't use these.
  flatbuffers::DetachedBuffer Release() = delete;
  flatbuffers::DetachedBuffer ReleaseBufferPointer() = delete;
};

}  // namespace deno

#endif  // FLATBUFFER_BUILDER_H_
