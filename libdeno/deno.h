// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef DENO_H_
#define DENO_H_
#include <stddef.h>
#include <stdint.h>
// Neither Rust nor Go support calling directly into C++ functions, therefore
// the public interface to libdeno is done in C.
#ifdef __cplusplus
extern "C" {
#endif

// Data that gets transmitted.
typedef struct {
  uint8_t* alloc_ptr;  // Start of memory allocation (returned from `malloc()`).
  size_t alloc_len;    // Length of the memory allocation.
  uint8_t* data_ptr;   // Start of logical contents (within the allocation).
  size_t data_len;     // Length of logical contents.
} deno_buf;

struct deno_s;
typedef struct deno_s Deno;

// A callback to receive a message from deno.send javascript call.
// buf is valid only for the lifetime of the call.
typedef void (*deno_recv_cb)(Deno* d, deno_buf buf);

void deno_init();
const char* deno_v8_version();
void deno_set_v8_flags(int* argc, char** argv);

Deno* deno_new(void* data, deno_recv_cb cb);
void deno_delete(Deno* d);

// Returns the void* data provided in deno_new.
void* deno_get_data(Deno*);

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
int deno_send(Deno* d, deno_buf buf);

// Call this inside a deno_recv_cb to respond synchronously to messages.
// If this is not called during the life time of a deno_recv_cb callback
// the deno.send() call in javascript will return null.
// After calling deno_set_response(), the caller no longer owns `buf` and must
// not access it; deno_set_response() is responsible for releasing it's memory.
void deno_set_response(Deno* d, deno_buf buf);

const char* deno_last_exception(Deno* d);

void deno_terminate_execution(Deno* d);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // DENO_H_
