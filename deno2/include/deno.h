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
typedef deno_buf (*RecvCallback)(Deno* d, deno_buf buf);

void deno_init();
const char* deno_v8_version();
void deno_set_flags(int* argc, char** argv);

// Constructor
Deno* deno_new(void* data, RecvCallback cb);

// Returns nonzero on error.
// Get error text with deno_last_exception().
int deno_load(Deno* d, const char* name_s, const char* source_s);

// Returns nonzero on error.
int deno_send(Deno* d, deno_buf buf);

const char* deno_last_exception(Deno* d);

void deno_dispose(Deno* d);
void deno_terminate_execution(Deno* d);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // INCLUDE_DENO_H_
