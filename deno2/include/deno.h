// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#ifndef INCLUDE_DENO_H_
#define INCLUDE_DENO_H_
// Neither Rust nor Go support calling directly into C++ functions, therefore
// the public interface to libdeno is done in C.
#ifdef __cplusplus
extern "C" {
#endif

// Data that gets transmitted.
typedef struct {
  const char* data;
  size_t len;
} deno_buf;

struct deno_s;
typedef struct deno_s Deno;

// A callback to receive a message from deno_pub javascript call.
// buf is valid only for the lifetime of the call.
typedef void (*deno_sub_cb)(Deno* d, const char* channel, deno_buf buf);

void deno_init();
const char* deno_v8_version();
void deno_set_flags(int* argc, char** argv);

Deno* deno_new(void* data, deno_sub_cb cb);
void deno_delete(Deno* d);

// Returns false on error.
// Get error text with deno_last_exception().
bool deno_execute(Deno* d, const char* js_filename, const char* js_source);

// Routes message to the javascript callback set with deno_sub(). A false return
// value indicates error. Check deno_last_exception() for exception text.
bool deno_pub(Deno* d, const char* channel, deno_buf buf);

// Call this inside a deno_sub_cb to respond synchronously to messages.
// If this is not called during the life time of a deno_sub_cb callback
// the denoPub() call in javascript will return null.
void deno_set_response(Deno* d, deno_buf buf);

const char* deno_last_exception(Deno* d);

void deno_terminate_execution(Deno* d);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // INCLUDE_DENO_H_
