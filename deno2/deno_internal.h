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
  std::string last_exception;
  v8::Persistent<v8::Function> recv;
  v8::Persistent<v8::Context> context;
  deno_recv_cb cb;
  void* data;
};
}

namespace deno {

void Print(const v8::FunctionCallbackInfo<v8::Value>& args);
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args);
void Send(const v8::FunctionCallbackInfo<v8::Value>& args);
static intptr_t external_references[] = {reinterpret_cast<intptr_t>(Print),
                                         reinterpret_cast<intptr_t>(Recv),
                                         reinterpret_cast<intptr_t>(Send), 0};

Deno* NewFromSnapshot(void* data, deno_recv_cb cb);

v8::StartupData MakeSnapshot(v8::StartupData* prev_natives_blob,
                             v8::StartupData* prev_snapshot_blob,
                             const char* js_filename, const char* js_source);

void AddIsolate(Deno* d, v8::Isolate* isolate);

}  // namespace deno
#endif  // DENO_INTERNAL_H_
