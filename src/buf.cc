
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "internal.h"

const deno_buf deno_buf_new(size_t len, bool zero_init) {
  auto ptr_void = zero_init ? calloc(1, len) : malloc(len);
  auto ptr = reinterpret_cast<uint8_t*>(ptr_void);
  if (ptr == nullptr) {
    fprintf(stderr, "deno_buf_new: out of memory\n");
    abort();
  }
  return deno_buf_new_raw(ptr, len, ptr, len);
}

void deno_buf_delete(deno_buf* buf) {
  if (buf->alloc_ptr != nullptr) {
    free(reinterpret_cast<void*>(buf->alloc_ptr));
  }
  deno_buf_delete_raw(buf);
}

const deno_buf deno_buf_move(deno_buf* source_buf) {
  const deno_buf target_buf =
      deno_buf_new_raw(source_buf->alloc_ptr, source_buf->alloc_len,
                       source_buf->data_ptr, source_buf->data_len);
  deno_buf_delete_raw(source_buf);
  return target_buf;
}

void deno_buf_move_into(deno_buf* target_buf, deno_buf* source_buf) {
  deno_buf_delete(target_buf);
  memcpy(target_buf, source_buf, sizeof *source_buf);
  deno_buf_delete_raw(source_buf);
}

bool deno_buf_is_null(const deno_buf* buf) {
  return buf->alloc_ptr == nullptr && buf->data_ptr == nullptr;
}

const deno_buf deno_buf_new_raw(uint8_t* alloc_ptr, size_t alloc_len,
                                uint8_t* data_ptr, size_t data_len) {
  const deno_buf buf{alloc_ptr, alloc_len, data_ptr, data_len};
  return buf;
}

void deno_buf_delete_raw(deno_buf* buf) { memset(buf, 0, sizeof *buf); }

#ifdef __cplusplus
deno_buf::~deno_buf() {
  if (!deno_buf_is_null(this)) {
    fprintf(stderr,
            "Buffer leaked. Remember to finalize every buffer with "
            "deno_buf_delete().");
    *(volatile char*)nullptr = 3;
    abort();
  }
}
#endif
