// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef INTERNAL_H_
#define INTERNAL_H_

#include <map>
#include <string>
#include "deno.h"
#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

namespace deno {

// deno_s = Wrapped Isolate.
class DenoIsolate {
 public:
  DenoIsolate(deno_buf snapshot, deno_recv_cb cb)
      : isolate_(nullptr),
        current_args_(nullptr),
        global_import_buf_ptr_(nullptr),
        pending_promise_events_(0),
        cb_(cb),
        next_req_id_(0),
        user_data_(nullptr) {
    if (snapshot.data_ptr) {
      snapshot_.data = reinterpret_cast<const char*>(snapshot.data_ptr);
      snapshot_.raw_size = static_cast<int>(snapshot.data_len);
    }
  }

  void AddIsolate(v8::Isolate* isolate);

  v8::Isolate* isolate_;
  // Put v8::Isolate::CreateParams here..
  const v8::FunctionCallbackInfo<v8::Value>* current_args_;
  void* global_import_buf_ptr_;
  int32_t pending_promise_events_;
  deno_recv_cb cb_;
  int32_t next_req_id_;
  void* user_data_;

  v8::Persistent<v8::Context> context_;
  std::map<int32_t, v8::Persistent<v8::Value>> async_data_map_;
  std::string last_exception_;
  v8::Persistent<v8::Function> recv_;
  v8::Persistent<v8::Function> global_error_handler_;
  v8::Persistent<v8::Function> promise_reject_handler_;
  v8::Persistent<v8::Function> promise_error_examiner_;
  v8::StartupData snapshot_;
  v8::Persistent<v8::ArrayBuffer> global_import_buf_;
};

class UserDataScope {
  DenoIsolate* deno;
  void* prev_data;
  void* data;  // Not necessary; only for sanity checking.

 public:
  UserDataScope(DenoIsolate* deno_, void* data_) : deno(deno_), data(data_) {
    CHECK(deno->user_data_ == nullptr || deno->user_data_ == data_);
    prev_data = deno->user_data_;
    deno->user_data_ = data;
  }

  ~UserDataScope() {
    CHECK(deno->user_data_ == data);
    deno->user_data_ = prev_data;
  }
};

struct InternalFieldData {
  uint32_t data;
};

void Print(const v8::FunctionCallbackInfo<v8::Value>& args);
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args);
void Send(const v8::FunctionCallbackInfo<v8::Value>& args);
void SetGlobalErrorHandler(const v8::FunctionCallbackInfo<v8::Value>& args);
void SetPromiseRejectHandler(const v8::FunctionCallbackInfo<v8::Value>& args);
void SetPromiseErrorExaminer(const v8::FunctionCallbackInfo<v8::Value>& args);
static intptr_t external_references[] = {
    reinterpret_cast<intptr_t>(Print),
    reinterpret_cast<intptr_t>(Recv),
    reinterpret_cast<intptr_t>(Send),
    reinterpret_cast<intptr_t>(SetGlobalErrorHandler),
    reinterpret_cast<intptr_t>(SetPromiseRejectHandler),
    reinterpret_cast<intptr_t>(SetPromiseErrorExaminer),
    0};

Deno* NewFromSnapshot(void* user_data, deno_recv_cb cb);

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context,
                       const char* js_filename, const std::string& js_source,
                       const std::string* source_map);

void HandleException(v8::Local<v8::Context> context,
                     v8::Local<v8::Value> exception);

void DeserializeInternalFields(v8::Local<v8::Object> holder, int index,
                               v8::StartupData payload, void* data);

v8::Local<v8::Uint8Array> ImportBuf(DenoIsolate* d, deno_buf buf);

void DeleteDataRef(DenoIsolate* d, int32_t req_id);

bool Execute(v8::Local<v8::Context> context, const char* js_filename,
             const char* js_source);

}  // namespace deno

extern "C" {
// This is just to workaround the linker.
struct deno_s {
  deno::DenoIsolate isolate;
};
}

#endif  // INTERNAL_H_
