// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#ifndef INCLUDE_DENO_H_
#define INCLUDE_DENO_H_

#include <string>
#include "v8/include/v8.h"

namespace deno {

// Data that gets transmitted.
struct buf_s {
  void* data;
  size_t len;
};
typedef struct buf_s DenoBuf;

struct deno_s;
typedef struct deno_s Deno;

// The callback from V8 when data is sent.
typedef DenoBuf (*RecvCallback)(Deno* d, DenoBuf buf);

void v8_init();
const char* v8_version();
void v8_set_flags(int* argc, char** argv);

// Constructors:
Deno* deno_from_snapshot(v8::StartupData* blob, void* data, RecvCallback cb);

v8::StartupData make_snapshot(v8::StartupData* prev_natives_blob,
                              v8::StartupData* prev_snapshot_blob,
                              const char* js_filename, const char* js_source);

void* deno_get_data();

// Returns nonzero on error.
// Get error text with deno_last_exception().
int deno_load(Deno* d, const char* name_s, const char* source_s);

// Returns nonzero on error.
int deno_send(Deno* d, DenoBuf buf);

const char* deno_last_exception(Deno* d);

void deno_dispose(Deno* d);
void deno_terminate_execution(Deno* d);

}  // namespace deno

#endif  // INCLUDE_DENO_H_
