// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#ifndef INTERNAL_H_
#define INTERNAL_H_

#include <string>
#include "deno.h"
#include "third_party/v8/include/v8.h"

extern "C" {
// deno_s = Wrapped Isolate.
struct deno_s {
  v8::Isolate* isolate;
  const v8::FunctionCallbackInfo<v8::Value>* currentArgs;
  std::string last_exception;
  v8::Persistent<v8::Function> recv;
  v8::Persistent<v8::Context> context;
  deno_recv_cb cb;
  void* data;
};
// TODO(ry) Remove these when we call deno_reply_start from Rust.
char** deno_argv();
int deno_argc();
}

namespace deno {

struct InternalFieldData {
  uint32_t data;
};

void Print(const v8::FunctionCallbackInfo<v8::Value>& args);
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args);
void Send(const v8::FunctionCallbackInfo<v8::Value>& args);
static intptr_t external_references[] = {reinterpret_cast<intptr_t>(Print),
                                         reinterpret_cast<intptr_t>(Recv),
                                         reinterpret_cast<intptr_t>(Send), 0};

Deno* NewFromSnapshot(void* data, deno_recv_cb cb);

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context,
                       const char* js_filename, const char* js_source);

void AddIsolate(Deno* d, v8::Isolate* isolate);

}  // namespace deno
#endif  // INTERNAL_H_
