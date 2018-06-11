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
  void* data;
  size_t len;
} deno_buf;

struct deno_s;
typedef struct deno_s Deno;

// The callback from V8 when data is sent.
typedef deno_buf (*deno_sub_cb)(Deno* d, deno_buf buf);

void deno_init();
const char* deno_v8_version();
void deno_set_flags(int* argc, char** argv);

// Constructor
Deno* deno_new(void* data, deno_sub_cb cb);

// Returns false on error.
// Get error text with deno_last_exception().
bool deno_execute(Deno* d, const char* js_filename, const char* js_source);

// Returns false on error.
// Get error text with deno_last_exception().
bool deno_pub(Deno* d, deno_buf buf);

const char* deno_last_exception(Deno* d);

void deno_dispose(Deno* d);
void deno_terminate_execution(Deno* d);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // INCLUDE_DENO_H_
