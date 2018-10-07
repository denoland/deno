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

// A callback to receive a message from a libdeno.send() javascript call.
// control_buf is valid for only for the lifetime of this callback.
// data_buf is valid until deno_respond() is called.
typedef void (*deno_recv_cb)(Deno* d, int32_t req_id, deno_buf control_buf,
                             deno_buf data_buf);

void deno_init();
const char* deno_v8_version();
void deno_set_v8_flags(int* argc, char** argv);

Deno* deno_new(deno_recv_cb cb);
void deno_delete(Deno* d);

// Returns the void* user_data provided in deno_new.
void* deno_get_data(Deno*);

// Returns false on error.
// Get error text with deno_last_exception().
// 0 = fail, 1 = success
int deno_execute(Deno* d, void* user_data, const char* js_filename,
                 const char* js_source);

// deno_respond sends up to one message back for every deno_recv_cb made.
//
// If this is called during deno_recv_cb, the issuing libdeno.send() in
// javascript will synchronously return the specified buf as an ArrayBuffer (or
// null if buf is empty).
//
// If this is called after deno_recv_cb has returned, the deno_respond
// will call into the JS callback specified by libdeno.recv().
//
// (Ideally, but not currently: After calling deno_respond(), the caller no
// longer owns `buf` and must not use it; deno_respond() is responsible for
// releasing its memory.)
//
// Calling this function more than once with the same req_id will result in
// an error.
//
// A non-zero return value, means a JS exception was encountered during the
// libdeno.recv() callback. Check deno_last_exception() for exception text.
int deno_respond(Deno* d, void* user_data, int32_t req_id, deno_buf buf);

const char* deno_last_exception(Deno* d);

void deno_terminate_execution(Deno* d);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // DENO_H_
