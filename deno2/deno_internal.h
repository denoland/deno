// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#ifndef DENO_INTERNAL_H_
#define DENO_INTERNAL_H_

#include <string>
#include "include/deno.h"
#include "v8/include/v8.h"

extern "C" {
// deno_s = Wrapped Isolate.
struct deno_s {
  v8::Isolate* isolate;
  const v8::FunctionCallbackInfo<v8::Value>* currentArgs;
  std::string last_exception;
  v8::Persistent<v8::Function> sub;
  v8::Persistent<v8::Context> context;
  deno_sub_cb cb;
  void* data;
};
}

namespace deno {

struct InternalFieldData {
  uint32_t data;
};

void Print(const v8::FunctionCallbackInfo<v8::Value>& args);
void Sub(const v8::FunctionCallbackInfo<v8::Value>& args);
void Pub(const v8::FunctionCallbackInfo<v8::Value>& args);
static intptr_t external_references[] = {reinterpret_cast<intptr_t>(Print),
                                         reinterpret_cast<intptr_t>(Sub),
                                         reinterpret_cast<intptr_t>(Pub), 0};

Deno* NewFromSnapshot(void* data, deno_sub_cb cb);

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context,
                       const char* js_filename, const char* js_source);

v8::StartupData MakeSnapshot(v8::StartupData* prev_natives_blob,
                             v8::StartupData* prev_snapshot_blob,
                             const char* js_filename, const char* js_source);

void AddIsolate(Deno* d, v8::Isolate* isolate);

}  // namespace deno
#endif  // DENO_INTERNAL_H_
