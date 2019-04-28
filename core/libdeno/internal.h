// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#ifndef INTERNAL_H_
#define INTERNAL_H_

#include <map>
#include <string>
#include <utility>
#include <vector>

#include "buffer.h"
#include "deno.h"
#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

namespace deno {

struct ModuleInfo {
  bool main;
  std::string name;
  v8::Persistent<v8::Module> handle;
  std::vector<std::string> import_specifiers;

  ModuleInfo(v8::Isolate* isolate, v8::Local<v8::Module> module, bool main_,
             const char* name_, std::vector<std::string> import_specifiers_)
      : main(main_), name(name_), import_specifiers(import_specifiers_) {
    handle.Reset(isolate, module);
  }
};

// deno_s = Wrapped Isolate.
class DenoIsolate {
 public:
  explicit DenoIsolate(deno_config config)
      : isolate_(nullptr),
        locker_(nullptr),
        shared_(config.shared),
        current_args_(nullptr),
        snapshot_creator_(nullptr),
        global_import_buf_ptr_(nullptr),
        recv_cb_(config.recv_cb),
        user_data_(nullptr),
        resolve_cb_(nullptr),
        has_snapshotted_(false) {
    if (config.load_snapshot.data_ptr) {
      snapshot_.data =
          reinterpret_cast<const char*>(config.load_snapshot.data_ptr);
      snapshot_.raw_size = static_cast<int>(config.load_snapshot.data_len);
    }
  }

  ~DenoIsolate() {
    shared_ab_.Reset();
    if (locker_) {
      delete locker_;
    }
    if (snapshot_creator_) {
      // TODO(ry) V8 has a strange assert which prevents a SnapshotCreator from
      // being deallocated if it hasn't created a snapshot yet.
      // https://github.com/v8/v8/blob/73212783fbd534fac76cc4b66aac899c13f71fc8/src/api.cc#L603
      // If that assert is removed, this if guard could be removed.
      // WARNING: There may be false positive LSAN errors here.
      if (has_snapshotted_) {
        delete snapshot_creator_;
      }
    } else {
      isolate_->Dispose();
    }
  }

  static inline DenoIsolate* FromIsolate(v8::Isolate* isolate) {
    return static_cast<DenoIsolate*>(isolate->GetData(0));
  }

  void AddIsolate(v8::Isolate* isolate);

  deno_mod RegisterModule(bool main, const char* name, const char* source);
  void ClearModules();

  ModuleInfo* GetModuleInfo(deno_mod id) {
    if (id == 0) {
      return nullptr;
    }
    auto it = mods_.find(id);
    if (it != mods_.end()) {
      return &it->second;
    } else {
      return nullptr;
    }
  }

  v8::Isolate* isolate_;
  v8::Locker* locker_;
  deno_buf shared_;
  const v8::FunctionCallbackInfo<v8::Value>* current_args_;
  v8::SnapshotCreator* snapshot_creator_;
  void* global_import_buf_ptr_;
  deno_recv_cb recv_cb_;
  void* user_data_;

  std::map<deno_mod, ModuleInfo> mods_;
  std::map<std::string, deno_mod> mods_by_name_;
  deno_resolve_cb resolve_cb_;

  v8::Persistent<v8::Context> context_;
  std::map<int, v8::Persistent<v8::Value>> pending_promise_map_;
  std::string last_exception_;
  v8::Persistent<v8::Function> recv_;
  v8::StartupData snapshot_;
  v8::Persistent<v8::ArrayBuffer> global_import_buf_;
  v8::Persistent<v8::SharedArrayBuffer> shared_ab_;
  bool has_snapshotted_;
};

class UserDataScope {
  DenoIsolate* deno_;
  void* prev_data_;
  void* data_;  // Not necessary; only for sanity checking.

 public:
  UserDataScope(DenoIsolate* deno, void* data) : deno_(deno), data_(data) {
    CHECK(deno->user_data_ == nullptr || deno->user_data_ == data_);
    prev_data_ = deno->user_data_;
    deno->user_data_ = data;
  }

  ~UserDataScope() {
    CHECK(deno_->user_data_ == data_);
    deno_->user_data_ = prev_data_;
  }
};

struct InternalFieldData {
  uint32_t data;
};

static inline v8::Local<v8::String> v8_str(const char* x) {
  return v8::String::NewFromUtf8(v8::Isolate::GetCurrent(), x,
                                 v8::NewStringType::kNormal)
      .ToLocalChecked();
}

void Print(const v8::FunctionCallbackInfo<v8::Value>& args);
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args);
void Send(const v8::FunctionCallbackInfo<v8::Value>& args);
void EvalContext(const v8::FunctionCallbackInfo<v8::Value>& args);
void ErrorToJSON(const v8::FunctionCallbackInfo<v8::Value>& args);
void Shared(v8::Local<v8::Name> property,
            const v8::PropertyCallbackInfo<v8::Value>& info);
void MessageCallback(v8::Local<v8::Message> message, v8::Local<v8::Value> data);
static intptr_t external_references[] = {
    reinterpret_cast<intptr_t>(Print),
    reinterpret_cast<intptr_t>(Recv),
    reinterpret_cast<intptr_t>(Send),
    reinterpret_cast<intptr_t>(EvalContext),
    reinterpret_cast<intptr_t>(ErrorToJSON),
    reinterpret_cast<intptr_t>(Shared),
    reinterpret_cast<intptr_t>(MessageCallback),
    0};

static const deno_buf empty_buf = {nullptr, 0, nullptr, 0};
static const deno_snapshot empty_snapshot = {nullptr, 0};

Deno* NewFromSnapshot(void* user_data, deno_recv_cb cb);

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context);

void DeserializeInternalFields(v8::Local<v8::Object> holder, int index,
                               v8::StartupData payload, void* data);

v8::StartupData SerializeInternalFields(v8::Local<v8::Object> holder, int index,
                                        void* data);

v8::Local<v8::Uint8Array> ImportBuf(DenoIsolate* d, deno_buf buf);

bool Execute(v8::Local<v8::Context> context, const char* js_filename,
             const char* js_source);
bool ExecuteMod(v8::Local<v8::Context> context, const char* js_filename,
                const char* js_source, bool resolve_only);

}  // namespace deno

extern "C" {
// This is just to workaround the linker.
struct deno_s {
  deno::DenoIsolate isolate;
};
}

#endif  // INTERNAL_H_
