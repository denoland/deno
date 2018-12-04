/// Copyright 2012 the V8 project authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include <algorithm>
#include <fstream>
#include <unordered_map>
#include <utility>
#include <vector>

#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/d8.h"
#include "third_party/v8/src/objects.h"
#include "third_party/v8/src/objects/string.h"
#include "third_party/v8/src/utils.h"

#include "worker.h"
#include "file_util.h"

const int kMaxWorkers = 100;

namespace deno {

static v8::Local<v8::Value> Throw(v8::Isolate* isolate, const char* message) {
  return isolate->ThrowException(
      v8::String::NewFromUtf8(isolate, message, v8::NewStringType::kNormal)
          .ToLocalChecked());
}

static v8::Local<v8::Value> GetValue(v8::Isolate* isolate, v8::Local<v8::Context> context,
                             v8::Local<v8::Object> object, const char* property) {
  v8::Local<v8::String> v8_str =
      v8::String::NewFromUtf8(isolate, property, v8::NewStringType::kNormal)
          .ToLocalChecked();
  return object->Get(context, v8_str).ToLocalChecked();
}

class ExternalOwningOneByteStringResource
    : public v8::String::ExternalOneByteStringResource {
 public:
  ExternalOwningOneByteStringResource() : length_(0) {}
  ExternalOwningOneByteStringResource(std::unique_ptr<const char[]> data,
                                      size_t length)
      : data_(std::move(data)), length_(length) {}
  const char* data() const override { return data_.get(); }
  size_t length() const override { return length_; }

 private:
  std::unique_ptr<const char[]> data_;
  size_t length_;
};


// Reads a file into a v8 string.
v8::Local<v8::String> ReadFile(v8::Isolate* isolate, const char* name) {
  int size = 0;

  std::string file_contents;
  CHECK(deno::ReadFileToString(name, &file_contents));
  char* chars = (char*)file_contents.c_str();
  size = (int)strlen(chars);
  if (chars == nullptr) return v8::Local<v8::String>();
  v8::Local<v8::String> result;

  if (i::FLAG_use_external_strings && i::String::IsAscii(chars, size)) {
    v8::String::ExternalOneByteStringResource* resource =
        new ExternalOwningOneByteStringResource(
            std::unique_ptr<const char[]>(chars), size);
    result = v8::String::NewExternalOneByte(isolate, resource).ToLocalChecked();
  } else {
    result = v8::String::NewFromUtf8(isolate, chars, v8::NewStringType::kNormal, size)
                 .ToLocalChecked();
    delete[] chars;
  }
  return result;
}

v8::base::LazyMutex workers_mutex_;
bool allow_new_workers_ = true;
std::vector<deno::Worker*> workers_;

void WorkerNew(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  v8::HandleScope handle_scope(isolate);
  if (args.Length() < 1 || !args[0]->IsString()) {
    Throw(args.GetIsolate(), "1st argument must be string");
    return;
  }

  // d8 honors `options={type: string}`, which means the first argument is
  // not a filename but string of script to be run.
  bool load_from_file = true;
  if (args.Length() > 1 && args[1]->IsObject()) {
    v8::Local<v8::Object> object = args[1].As<v8::Object>();
    v8::Local<v8::Context> context = isolate->GetCurrentContext();
    v8::Local<v8::Value> value = GetValue(args.GetIsolate(), context, object, "type");
    if (value->IsString()) {
      v8::Local<v8::String> worker_type = value->ToString(context).ToLocalChecked();
      v8::String::Utf8Value str(isolate, worker_type);
      if (strcmp("string", *str) == 0) {
        load_from_file = false;
      } else if (strcmp("classic", *str) == 0) {
        load_from_file = true;
      } else {
        Throw(args.GetIsolate(), "Unsupported worker type");
        return;
      }
    }
  }

  v8::Local<v8::Value> source;
  if (load_from_file) {
    v8::String::Utf8Value filename(args.GetIsolate(), args[0]);
    source = ReadFile(args.GetIsolate(), *filename);
    if (source.IsEmpty()) {
      Throw(args.GetIsolate(), "Error loading worker script");
      return;
    }
  } else {
    source = args[0];
  }

  if (!args.IsConstructCall()) {
    Throw(args.GetIsolate(), "Worker must be constructed with new");
    return;
  }

  {
    v8::base::LockGuard<v8::base::Mutex> lock_guard(workers_mutex_.Pointer());
    if (workers_.size() >= kMaxWorkers) {
      Throw(args.GetIsolate(), "Too many workers, I won't let you create more");
      return;
    }

    // Initialize the embedder field to nullptr; if we return early without
    // creating a new Worker (because the main thread is terminating) we can
    // early-out from the instance calls.
    args.Holder()->SetAlignedPointerInInternalField(0, nullptr);

    if (!allow_new_workers_) return;

    Worker* worker = new Worker;
    args.Holder()->SetAlignedPointerInInternalField(0, worker);
    workers_.push_back(worker);

    v8::String::Utf8Value script(args.GetIsolate(), source);
    if (!*script) {
      Throw(args.GetIsolate(), "Can't get worker script");
      return;
    }
    worker->StartExecuteInThread(*script);
  }
}


// void WorkerPostMessage(const v8::FunctionCallbackInfo<v8::Value>& args) {
//   v8::Isolate* isolate = args.GetIsolate();
//   v8::HandleScope handle_scope(isolate);

//   if (args.Length() < 1) {
//     Throw(isolate, "Invalid argument");
//     return;
//   }

//   Worker* worker = GetWorkerFromInternalField(isolate, args.Holder());
//   if (!worker) {
//     return;
//   }

//   v8::Local<Value> message = args[0];
//   v8::Local<Value> transfer =
//       args.Length() >= 2 ? args[1] : v8::Local<Value>::Cast(Undefined(isolate));
//   std::unique_ptr<SerializationData> data =
//       SerializeValue(isolate, message, transfer);
//   if (data) {
//     worker->PostMessage(std::move(data));
//   }
// }


// void WorkerGetMessage(const v8::FunctionCallbackInfo<v8::Value>& args) {
//   v8::Isolate* isolate = args.GetIsolate();
//   v8::HandleScope handle_scope(isolate);
//   Worker* worker = GetWorkerFromInternalField(isolate, args.Holder());
//   if (!worker) {
//     return;
//   }

//   std::unique_ptr<SerializationData> data = worker->GetMessage();
//   if (data) {
//     v8::Local<Value> value;
//     if (DeserializeValue(isolate, std::move(data)).ToLocal(&value)) {
//       args.GetReturnValue().Set(value);
//     }
//   }
// }


// void WorkerTerminate(const v8::FunctionCallbackInfo<v8::Value>& args) {
//   Isolate* isolate = args.GetIsolate();
//   HandleScope handle_scope(isolate);
//   Worker* worker = GetWorkerFromInternalField(isolate, args.Holder());
//   if (!worker) {
//     return;
//   }

//   worker->Terminate();
// }

} // namespace "deno"

