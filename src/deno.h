// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef DENO_H_
#define DENO_H_
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// Neither Rust nor Go support calling directly into C++ functions, therefore
// the public interface to libdeno is done in C.
#ifdef __cplusplus
extern "C" {
#endif

// Data that gets transmitted.
typedef struct deno_buf {
  uint8_t* const alloc_ptr;  // Start of memory allocation. Read-only.
  const size_t alloc_len;    // Length of the memory allocation. Read-only.
  uint8_t* data_ptr;  // Start of logical contents (within the allocation).
  size_t data_len;    // Length of logical contents.

#ifdef __cplusplus
  // Since deno_buf has a C abi, we can't rely on the destructor to run, so
  // the user has to call deno_buf_delete() explicitly. However, if we are in
  // C++, use the destructor to catch bugs.
  ~deno_buf();
  // Disallow pass-by-value.
  deno_buf(deno_buf&) = delete;
  deno_buf(const deno_buf&) = default;

#endif
} deno_buf;

// Clang will complain that the deno_buf_* functions have C linkage while
// `struct deno_buf` has a destructor. This is intentional -- suppress the
// warning.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wreturn-type-c-linkage"

static const deno_buf DENO_BUF_INIT = {nullptr, 0, nullptr, 0};

// Allocate a new buffer with backing storage.
const deno_buf deno_buf_new(size_t len, bool zero_init);
// Allocate new storage for an existing buffer. The `data_ptr` and `data_len`
// fields are initialized to use the entire allocation.
void deno_buf_delete(deno_buf* buf);
// Move ownership of a buffer. Returns a new buffer with the same contents as
// the source buffer. Afterwards, the source buffer is reset to null.
const deno_buf deno_buf_move(deno_buf* source_buf);
void deno_buf_move_into(deno_buf* target_buf, deno_buf* source_buf);
// Returns true if `buf` is a null buffer.
bool deno_buf_is_null(const deno_buf* buf);

// Directly manipulate deno_buf fields, without doing any memory management.
// This is for special use cases - don't use this unless unavoidable.
const deno_buf deno_buf_new_raw(uint8_t* alloc_ptr, size_t alloc_len,
                                uint8_t* data_ptr, size_t data_len);
void deno_buf_delete_raw(deno_buf* buf);

#pragma clang diagnostic pop

struct deno_s;
typedef struct deno_s Deno;

// A callback to receive a message from deno.send javascript call.
// buf is valid only for the lifetime of the call.
typedef void (*deno_recv_cb)(Deno* d, deno_buf* buf);

// This callback receives a deno_buf containing a message. It should return a
// message's cmd_id.
typedef uint32_t (*deno_cmd_id_cb)(const deno_buf* buf);

void deno_init();
const char* deno_v8_version();
void deno_set_flags(int* argc, char** argv);

Deno* deno_new(void* data, deno_recv_cb recv_cb, deno_cmd_id_cb cmd_id_cb);
void deno_delete(Deno* d);

// Returns the void* data provided in deno_new.
void* deno_get_data(Deno*);
// Returns true if we're running the VM and backend in separate threads.
bool deno_threads_enabled(Deno* d);

// Returns false on error.
// Get error text with deno_last_exception().
// 0 = fail, 1 = success
int deno_execute(Deno* d, const char* js_filename, const char* js_source);

// Routes message to the javascript callback set with deno.recv(). A false
// return value indicates error. Check deno_last_exception() for exception text.
// 0 = fail, 1 = success
// After calling deno_send(), the caller no longer owns `buf` and must not use
// it; deno_send() is responsible for releasing it's memory.
// TODO(piscisaureus) In C++ and/or Rust, use a smart pointer or similar to
// enforce this rule.
int deno_send(Deno* d, deno_buf* buf);

// Call this inside a deno_recv_cb to respond synchronously to messages.
// If this is not called during the life time of a deno_recv_cb callback
// the deno.send() call in javascript will return null.
// After calling deno_set_response(), the caller no longer owns `buf` and must
// not access it; deno_set_response() is responsible for releasing it's memory.
void deno_set_response(Deno* d, deno_buf* buf);

// Receive a message from javascript.
//   `buf`     : Pointer to a `struct deno_buf` which' properties will point
//               at the message if this function succeeds.
//   `timeout` : specifies how long deno_recv should block. -1 means never.
//   returns   :  -1 on error, 0 on timeout, 1 when succesful.
int deno_recv(Deno* d, deno_buf* buf, int64_t timeout);

const char* deno_last_exception(Deno* d);

void deno_terminate_execution(Deno* d);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // DENO_H_
