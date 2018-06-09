// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#ifndef DENO_H_
#define DENO_H_

#include <string>
#include "v8/include/v8.h"

// Data that gets transmitted.
struct buf_s {
  void* data;
  size_t len;
};
typedef struct buf_s DenoBuf;
// Deno = Wrapped Isolate.
struct deno_s;
typedef struct deno_s Deno;
// The callback from V8 when data is sent.
typedef DenoBuf (*RecvCallback)(Deno* d, DenoBuf buf);
struct deno_s {
  v8::Isolate* isolate;
  std::string last_exception;
  v8::Persistent<v8::Function> recv;
  v8::Persistent<v8::Context> context;
  RecvCallback cb;
  void* data;
};

void v8_init();
const char* v8_version();
void v8_set_flags(int* argc, char** argv);

// Constructors:
Deno* deno_new(void* data, RecvCallback cb);
Deno* deno_from_snapshot(v8::StartupData* blob, void* data, RecvCallback cb);

v8::StartupData deno_make_snapshot(const char* js_filename,
                                   const char* js_source);

void deno_add_isolate(Deno* d, v8::Isolate* isolate);
void* deno_get_data();

// Returns nonzero on error.
// Get error text with deno_last_exception().
int deno_load(Deno* d, const char* name_s, const char* source_s);

// Returns nonzero on error.
int deno_send(Deno* d, DenoBuf buf);

const char* deno_last_exception(Deno* d);

void deno_dispose(Deno* d);
void deno_terminate_execution(Deno* d);

#endif  // DENO_H_
