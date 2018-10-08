// Copyright 2018 the Deno authors. All rights reserved. MIT license.
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
  v8::Persistent<v8::Function> global_error_handler;
  v8::Persistent<v8::Context> context;
  v8::Persistent<v8::Map> async_data_map;
  deno_recv_cb cb;
  int32_t next_req_id;
  void* user_data;
};
}

namespace deno {

struct InternalFieldData {
  uint32_t data;
};

void Print(const v8::FunctionCallbackInfo<v8::Value>& args);
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args);
void Send(const v8::FunctionCallbackInfo<v8::Value>& args);
void SetGlobalErrorHandler(const v8::FunctionCallbackInfo<v8::Value>& args);
static intptr_t external_references[] = {
    reinterpret_cast<intptr_t>(Print), reinterpret_cast<intptr_t>(Recv),
    reinterpret_cast<intptr_t>(Send),
    reinterpret_cast<intptr_t>(SetGlobalErrorHandler), 0};

Deno* NewFromSnapshot(void* user_data, deno_recv_cb cb);

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context,
                       const char* js_filename, const std::string& js_source,
                       const std::string* source_map);

void AddIsolate(Deno* d, v8::Isolate* isolate);

}  // namespace deno
#endif  // INTERNAL_H_
