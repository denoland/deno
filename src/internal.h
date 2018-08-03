// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef INTERNAL_H_
#define INTERNAL_H_

#include <string>
#include "deno.h"
#include "mq.h"
#include "third_party/v8/include/v8.h"

extern "C" {
// deno_s = Wrapped Isolate.
struct deno_s {
  v8::Isolate* isolate;
  const v8::FunctionCallbackInfo<v8::Value>* current_args;
  const deno_buf* current_cmd;
  std::string last_exception;
  v8::Persistent<v8::Function> recv;
  v8::Persistent<v8::Context> context;
  deno_recv_cb recv_cb;
  deno_cmd_id_cb cmd_id_cb;
  void* data;
  deno::MessageQueue cmd_queue;  // JavaScript -> backend.
  deno::MessageQueue res_queue;  // Backend -> JavaScript
  bool threads_enabled;
};
// TODO(ry) Remove these when we call deno_reply_start from Rust.
char** deno_argv();
int deno_argc();
struct deno_s* deno_from_isolate(v8::Isolate* isolate);
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

void InitializeCommon(Deno* d, void* data, deno_recv_cb recv_cb,
                      deno_cmd_id_cb cmd_id_cb);

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context,
                       const char* js_filename, const std::string& js_source,
                       const std::string* source_map);

void AddIsolate(Deno* d, v8::Isolate* isolate);

}  // namespace deno
#endif  // INTERNAL_H_
