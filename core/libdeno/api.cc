// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <iostream>
#include <string>

#include "third_party/v8/include/libplatform/libplatform.h"
#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

#include "deno.h"
#include "exceptions.h"
#include "file_util.h"
#include "internal.h"

extern "C" {

Deno* deno_new_snapshotter(deno_config config) {
  CHECK(config.will_snapshot);
  // TODO(ry) Support loading snapshots before snapshotting.
  CHECK_NULL(config.load_snapshot.data_ptr);
  auto* creator = new v8::SnapshotCreator(deno::external_references);
  auto* isolate = creator->GetIsolate();
  auto* d = new deno::DenoIsolate(config);
  d->snapshot_creator_ = creator;
  d->AddIsolate(isolate);
  {
    v8::Locker locker(isolate);
    v8::Isolate::Scope isolate_scope(isolate);
    v8::HandleScope handle_scope(isolate);
    auto context = v8::Context::New(isolate);
    d->context_.Reset(isolate, context);

    creator->SetDefaultContext(context,
                               v8::SerializeInternalFieldsCallback(
                                   deno::SerializeInternalFields, nullptr));
    deno::InitializeContext(isolate, context);
  }
  return reinterpret_cast<Deno*>(d);
}

Deno* deno_new(deno_config config) {
  if (config.will_snapshot) {
    return deno_new_snapshotter(config);
  }
  deno::DenoIsolate* d = new deno::DenoIsolate(config);
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator = &deno::ArrayBufferAllocator::global();
  params.external_references = deno::external_references;

  if (config.load_snapshot.data_ptr) {
    params.snapshot_blob = &d->snapshot_;
  }

  v8::Isolate* isolate = v8::Isolate::New(params);
  d->AddIsolate(isolate);

  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  {
    v8::HandleScope handle_scope(isolate);
    auto context =
        v8::Context::New(isolate, nullptr, v8::MaybeLocal<v8::ObjectTemplate>(),
                         v8::MaybeLocal<v8::Value>(),
                         v8::DeserializeInternalFieldsCallback(
                             deno::DeserializeInternalFields, nullptr));
    if (!config.load_snapshot.data_ptr) {
      // If no snapshot is provided, we initialize the context with empty
      // main source code and source maps.
      deno::InitializeContext(isolate, context);
    }
    d->context_.Reset(isolate, context);
  }

  return reinterpret_cast<Deno*>(d);
}

deno::DenoIsolate* unwrap(Deno* d_) {
  return reinterpret_cast<deno::DenoIsolate*>(d_);
}

void deno_lock(Deno* d_) {
  auto* d = unwrap(d_);
  CHECK_NULL(d->locker_);
  d->locker_ = new v8::Locker(d->isolate_);
}

void deno_unlock(Deno* d_) {
  auto* d = unwrap(d_);
  CHECK_NOT_NULL(d->locker_);
  delete d->locker_;
  d->locker_ = nullptr;
}

deno_snapshot deno_snapshot_new(Deno* d_) {
  auto* d = unwrap(d_);
  CHECK_NOT_NULL(d->snapshot_creator_);
  d->ClearModules();
  d->context_.Reset();

  auto blob = d->snapshot_creator_->CreateBlob(
      v8::SnapshotCreator::FunctionCodeHandling::kKeep);
  d->has_snapshotted_ = true;
  return {reinterpret_cast<uint8_t*>(const_cast<char*>(blob.data)),
          blob.raw_size};
}

void deno_snapshot_delete(deno_snapshot snapshot) {
  delete[] snapshot.data_ptr;
}

static std::unique_ptr<v8::Platform> platform;

void deno_init() {
  if (platform.get() == nullptr) {
    platform = v8::platform::NewDefaultPlatform();
    v8::V8::InitializePlatform(platform.get());
    v8::V8::Initialize();
    // TODO(ry) This makes WASM compile synchronously. Eventually we should
    // remove this to make it work asynchronously too. But that requires getting
    // PumpMessageLoop and RunMicrotasks setup correctly.
    // See https://github.com/denoland/deno/issues/2544
    const char* argv[2] = {"", "--no-wasm-async-compilation"};
    int argc = 2;
    v8::V8::SetFlagsFromCommandLine(&argc, const_cast<char**>(argv), false);
  }
}

const char* deno_v8_version() { return v8::V8::GetVersion(); }

void deno_set_v8_flags(int* argc, char** argv) {
  v8::V8::SetFlagsFromCommandLine(argc, argv, true);
}

const char* deno_last_exception(Deno* d_) {
  auto* d = unwrap(d_);
  if (d->last_exception_.length() > 0) {
    return d->last_exception_.c_str();
  } else {
    return nullptr;
  }
}

void deno_execute(Deno* d_, void* user_data, const char* js_filename,
                  const char* js_source) {
  auto* d = unwrap(d_);
  deno::UserDataScope user_data_scope(d, user_data);
  auto* isolate = d->isolate_;
  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  auto context = d->context_.Get(d->isolate_);
  CHECK(!context.IsEmpty());
  deno::Execute(context, js_filename, js_source);
}

void deno_pinned_buf_delete(deno_pinned_buf* buf) {
  // The PinnedBuf destructor implicitly releases the ArrayBuffer reference.
  auto _ = deno::PinnedBuf(buf);
}

void deno_respond(Deno* d_, void* user_data, deno_buf buf) {
  auto* d = unwrap(d_);
  if (d->current_args_ != nullptr) {
    // Synchronous response.
    if (buf.data_ptr != nullptr) {
      auto ab = deno::ImportBuf(d, buf);
      d->current_args_->GetReturnValue().Set(ab);
    }
    d->current_args_ = nullptr;
    return;
  }

  // Asynchronous response.
  deno::UserDataScope user_data_scope(d, user_data);
  v8::Locker locker(d->isolate_);
  v8::Isolate::Scope isolate_scope(d->isolate_);
  v8::HandleScope handle_scope(d->isolate_);

  auto context = d->context_.Get(d->isolate_);
  v8::Context::Scope context_scope(context);

  v8::TryCatch try_catch(d->isolate_);

  auto recv_ = d->recv_.Get(d->isolate_);
  if (recv_.IsEmpty()) {
    d->last_exception_ = "Deno.core.recv has not been called.";
    return;
  }

  v8::Local<v8::Value> args[1];
  int argc = 0;

  if (buf.data_ptr != nullptr) {
    args[0] = deno::ImportBuf(d, buf);
    argc = 1;
  }

  auto v = recv_->Call(context, context->Global(), argc, args);

  if (try_catch.HasCaught()) {
    CHECK(v.IsEmpty());
    deno::HandleException(context, try_catch.Exception());
  }
}

void deno_check_promise_errors(Deno* d_) {
  auto* d = unwrap(d_);
  if (d->pending_promise_map_.size() > 0) {
    auto* isolate = d->isolate_;
    v8::Locker locker(isolate);
    v8::Isolate::Scope isolate_scope(isolate);
    v8::HandleScope handle_scope(isolate);
    auto context = d->context_.Get(d->isolate_);
    v8::Context::Scope context_scope(context);

    auto it = d->pending_promise_map_.begin();
    while (it != d->pending_promise_map_.end()) {
      auto error = it->second.Get(isolate);
      deno::HandleException(context, error);
      it = d->pending_promise_map_.erase(it);
    }
  }
}

void deno_delete(Deno* d_) {
  deno::DenoIsolate* d = reinterpret_cast<deno::DenoIsolate*>(d_);
  delete d;
}

void deno_terminate_execution(Deno* d_) {
  deno::DenoIsolate* d = reinterpret_cast<deno::DenoIsolate*>(d_);
  d->isolate_->TerminateExecution();
}
}
