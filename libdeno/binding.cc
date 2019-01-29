// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <iostream>
#include <string>

#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

#include "deno.h"
#include "exceptions.h"
#include "internal.h"

#define GLOBAL_IMPORT_BUF_SIZE 1024

namespace deno {

std::vector<InternalFieldData*> deserialized_data;

void DeserializeInternalFields(v8::Local<v8::Object> holder, int index,
                               v8::StartupData payload, void* data) {
  DCHECK_EQ(data, nullptr);
  if (payload.raw_size == 0) {
    holder->SetAlignedPointerInInternalField(index, nullptr);
    return;
  }
  InternalFieldData* embedder_field = new InternalFieldData{0};
  memcpy(embedder_field, payload.data, payload.raw_size);
  holder->SetAlignedPointerInInternalField(index, embedder_field);
  deserialized_data.push_back(embedder_field);
}

v8::StartupData SerializeInternalFields(v8::Local<v8::Object> holder, int index,
                                        void* data) {
  DCHECK_EQ(data, nullptr);
  InternalFieldData* embedder_field = static_cast<InternalFieldData*>(
      holder->GetAlignedPointerFromInternalField(index));
  if (embedder_field == nullptr) return {nullptr, 0};
  int size = sizeof(*embedder_field);
  char* payload = new char[size];
  // We simply use memcpy to serialize the content.
  memcpy(payload, embedder_field, size);
  return {payload, size};
}

void AddDataRef(DenoIsolate* d, int32_t req_id, v8::Local<v8::Value> data_v) {
  d->async_data_map_.emplace(std::piecewise_construct, std::make_tuple(req_id),
                             std::make_tuple(d->isolate_, data_v));
}

void DeleteDataRef(DenoIsolate* d, int32_t req_id) {
  // Delete persistent reference to data ArrayBuffer.
  auto it = d->async_data_map_.find(req_id);
  if (it != d->async_data_map_.end()) {
    it->second.Reset();
    d->async_data_map_.erase(it);
  }
}

// Extracts a C string from a v8::V8 Utf8Value.
const char* ToCString(const v8::String::Utf8Value& value) {
  return *value ? *value : "<string conversion failed>";
}

void PromiseRejectCallback(v8::PromiseRejectMessage promise_reject_message) {
  auto* isolate = v8::Isolate::GetCurrent();
  DenoIsolate* d = static_cast<DenoIsolate*>(isolate->GetData(0));
  DCHECK_EQ(d->isolate_, isolate);
  v8::HandleScope handle_scope(d->isolate_);
  auto error = promise_reject_message.GetValue();
  auto context = d->context_.Get(d->isolate_);
  auto promise = promise_reject_message.GetPromise();

  v8::Context::Scope context_scope(context);

  int promise_id = promise->GetIdentityHash();
  switch (promise_reject_message.GetEvent()) {
    case v8::kPromiseRejectWithNoHandler:
      // Insert the error into the pending_promise_map_ using the promise's id
      // as the key.
      d->pending_promise_map_.emplace(std::piecewise_construct,
                                      std::make_tuple(promise_id),
                                      std::make_tuple(d->isolate_, error));
      break;

    case v8::kPromiseHandlerAddedAfterReject:
      d->pending_promise_map_.erase(promise_id);
      break;

    case v8::kPromiseRejectAfterResolved:
      break;

    case v8::kPromiseResolveAfterResolved:
      // Should not warn. See #1272
      break;

    default:
      CHECK(false && "unreachable");
  }
}

void Print(const v8::FunctionCallbackInfo<v8::Value>& args) {
  CHECK_GE(args.Length(), 1);
  CHECK_LE(args.Length(), 3);
  auto* isolate = args.GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  auto context = d->context_.Get(d->isolate_);
  v8::HandleScope handle_scope(isolate);
  v8::String::Utf8Value str(isolate, args[0]);
  bool is_err =
      args.Length() >= 2 ? args[1]->BooleanValue(context).ToChecked() : false;
  bool prints_newline =
      args.Length() >= 3 ? args[2]->BooleanValue(context).ToChecked() : true;
  FILE* file = is_err ? stderr : stdout;
  fwrite(*str, sizeof(**str), str.length(), file);
  if (prints_newline) {
    fprintf(file, "\n");
  }
  fflush(file);
}

v8::Local<v8::Uint8Array> ImportBuf(DenoIsolate* d, deno_buf buf) {
  if (buf.alloc_ptr == nullptr) {
    // If alloc_ptr isn't set, we memcpy.
    // This is currently used for flatbuffers created in Rust.

    // To avoid excessively allocating new ArrayBuffers, we try to reuse a
    // single global ArrayBuffer. The caveat is that users must extract data
    // from it before the next tick. We only do this for ArrayBuffers less than
    // 1024 bytes.
    v8::Local<v8::ArrayBuffer> ab;
    void* data;
    if (buf.data_len > GLOBAL_IMPORT_BUF_SIZE) {
      // Simple case. We allocate a new ArrayBuffer for this.
      ab = v8::ArrayBuffer::New(d->isolate_, buf.data_len);
      data = ab->GetContents().Data();
    } else {
      // Fast case. We reuse the global ArrayBuffer.
      if (d->global_import_buf_.IsEmpty()) {
        // Lazily initialize it.
        DCHECK_EQ(d->global_import_buf_ptr_, nullptr);
        ab = v8::ArrayBuffer::New(d->isolate_, GLOBAL_IMPORT_BUF_SIZE);
        d->global_import_buf_.Reset(d->isolate_, ab);
        d->global_import_buf_ptr_ = ab->GetContents().Data();
      } else {
        DCHECK(d->global_import_buf_ptr_);
        ab = d->global_import_buf_.Get(d->isolate_);
      }
      data = d->global_import_buf_ptr_;
    }
    memcpy(data, buf.data_ptr, buf.data_len);
    auto view = v8::Uint8Array::New(ab, 0, buf.data_len);
    return view;
  } else {
    auto ab = v8::ArrayBuffer::New(
        d->isolate_, reinterpret_cast<void*>(buf.alloc_ptr), buf.alloc_len,
        v8::ArrayBufferCreationMode::kInternalized);
    auto view =
        v8::Uint8Array::New(ab, buf.data_ptr - buf.alloc_ptr, buf.data_len);
    return view;
  }
}

static deno_buf GetContents(v8::Isolate* isolate,
                            v8::Local<v8::ArrayBufferView> view) {
  auto ab = view->Buffer();
  auto contents = ab->GetContents();
  deno_buf buf;
  buf.alloc_ptr = reinterpret_cast<uint8_t*>(contents.Data());
  buf.alloc_len = contents.ByteLength();
  buf.data_ptr = buf.alloc_ptr + view->ByteOffset();
  buf.data_len = view->ByteLength();
  return buf;
}

// Sets the recv_ callback.
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  DCHECK_EQ(d->isolate_, isolate);

  v8::HandleScope handle_scope(isolate);

  if (!d->recv_.IsEmpty()) {
    isolate->ThrowException(v8_str("libdeno.recv_ already called."));
    return;
  }

  v8::Local<v8::Value> v = args[0];
  CHECK(v->IsFunction());
  v8::Local<v8::Function> func = v8::Local<v8::Function>::Cast(v);

  d->recv_.Reset(isolate, func);
}

void Send(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  DCHECK_EQ(d->isolate_, isolate);

  v8::Locker locker(d->isolate_);
  v8::HandleScope handle_scope(isolate);

  CHECK_EQ(d->current_args_, nullptr);  // libdeno.send re-entry forbidden.
  int32_t req_id = d->next_req_id_++;

  v8::Local<v8::Value> control_v = args[0];
  CHECK(control_v->IsArrayBufferView());
  deno_buf control =
      GetContents(isolate, v8::Local<v8::ArrayBufferView>::Cast(control_v));
  deno_buf data = {nullptr, 0u, nullptr, 0u};
  v8::Local<v8::Value> data_v;
  if (args.Length() == 2) {
    if (args[1]->IsArrayBufferView()) {
      data_v = args[1];
      data = GetContents(isolate, v8::Local<v8::ArrayBufferView>::Cast(data_v));
    }
  } else {
    CHECK_EQ(args.Length(), 1);
  }

  DCHECK_EQ(d->current_args_, nullptr);
  d->current_args_ = &args;

  d->recv_cb_(d->user_data_, req_id, control, data);

  if (d->current_args_ == nullptr) {
    // This indicates that deno_repond() was called already.
  } else {
    // Asynchronous.
    d->current_args_ = nullptr;
    // If the data ArrayBuffer was given, we must maintain a strong reference
    // to it until deno_respond is called.
    if (!data_v.IsEmpty()) {
      AddDataRef(d, req_id, data_v);
    }
  }
}

v8::Local<v8::Object> DenoIsolate::GetBuiltinModules() {
  v8::EscapableHandleScope handle_scope(isolate_);
  if (builtin_modules_.IsEmpty()) {
    builtin_modules_.Reset(isolate_, v8::Object::New(isolate_));
  }
  return handle_scope.Escape(builtin_modules_.Get(isolate_));
}

void BuiltinModules(v8::Local<v8::Name> property,
                    const v8::PropertyCallbackInfo<v8::Value>& info) {
  v8::Isolate* isolate = info.GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  DCHECK_EQ(d->isolate_, isolate);
  v8::Locker locker(d->isolate_);
  info.GetReturnValue().Set(d->GetBuiltinModules());
}

void Shared(v8::Local<v8::Name> property,
            const v8::PropertyCallbackInfo<v8::Value>& info) {
  v8::Isolate* isolate = info.GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  DCHECK_EQ(d->isolate_, isolate);
  v8::Locker locker(d->isolate_);
  v8::EscapableHandleScope handle_scope(isolate);
  if (d->shared_.data_ptr == nullptr) {
    return;
  }
  v8::Local<v8::ArrayBuffer> ab;
  if (d->shared_ab_.IsEmpty()) {
    // Lazily initialize the persistent external ArrayBuffer.
    ab = v8::ArrayBuffer::New(isolate, d->shared_.data_ptr, d->shared_.data_len,
                              v8::ArrayBufferCreationMode::kExternalized);
    d->shared_ab_.Reset(isolate, ab);
  }
  info.GetReturnValue().Set(ab);
}

v8::ScriptOrigin ModuleOrigin(v8::Local<v8::Value> resource_name,
                              v8::Isolate* isolate) {
  return v8::ScriptOrigin(resource_name, v8::Local<v8::Integer>(),
                          v8::Local<v8::Integer>(), v8::Local<v8::Boolean>(),
                          v8::Local<v8::Integer>(), v8::Local<v8::Value>(),
                          v8::Local<v8::Boolean>(), v8::Local<v8::Boolean>(),
                          v8::True(isolate));
}

void DenoIsolate::ClearModules() {
  for (auto it = module_map_.begin(); it != module_map_.end(); it++) {
    it->second.Reset();
  }
  module_map_.clear();
  for (auto it = module_info_map_.begin(); it != module_info_map_.end(); it++) {
    it->second.second.Reset();
  }
  module_info_map_.clear();
}

void DenoIsolate::RegisterModule(const char* filename,
                                 v8::Local<v8::Module> module) {
  int id = module->GetIdentityHash();

  module_map_.emplace(std::piecewise_construct, std::make_tuple(filename),
                      std::make_tuple(isolate_, module));

  // Identity hash is not necessarily unique
  // Therefore, we store a persistent handle along with filenames
  // such that we can compare the identites and select the correct module
  module_info_map_.emplace(
      std::piecewise_construct, std::make_tuple(id),
      std::make_tuple(std::piecewise_construct, std::make_tuple(filename),
                      std::make_tuple(isolate_, module)));
}

v8::MaybeLocal<v8::Module> CompileModule(v8::Local<v8::Context> context,
                                         const char* js_filename,
                                         v8::Local<v8::String> source_text) {
  auto* isolate = context->GetIsolate();

  v8::Isolate::Scope isolate_scope(isolate);
  v8::EscapableHandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto origin = ModuleOrigin(v8_str(js_filename, true), isolate);
  v8::ScriptCompiler::Source source(source_text, origin);

  auto maybe_module = v8::ScriptCompiler::CompileModule(isolate, &source);

  if (!maybe_module.IsEmpty()) {
    auto module = maybe_module.ToLocalChecked();
    CHECK_EQ(v8::Module::kUninstantiated, module->GetStatus());
    DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
    d->RegisterModule(js_filename, module);
  }

  return handle_scope.EscapeMaybe(maybe_module);
}

v8::MaybeLocal<v8::Module> ResolveCallback(v8::Local<v8::Context> context,
                                           v8::Local<v8::String> specifier,
                                           v8::Local<v8::Module> referrer) {
  auto* isolate = context->GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);

  v8::Isolate::Scope isolate_scope(isolate);
  v8::EscapableHandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  v8::String::Utf8Value specifier_utf8val(isolate, specifier);
  const char* specifier_cstr = ToCString(specifier_utf8val);

  auto builtin_modules = d->GetBuiltinModules();
  bool has_builtin = builtin_modules->Has(context, specifier).ToChecked();
  if (has_builtin) {
    auto val = builtin_modules->Get(context, specifier).ToLocalChecked();
    CHECK(val->IsObject());
    auto obj = val->ToObject(isolate);

    // In order to export obj as a module, we must iterate over its properties
    // and export them each individually.
    // TODO Find a better way to do this.
    std::string src = "let globalEval = eval\nlet g = globalEval('this');\n";
    auto names = obj->GetOwnPropertyNames(context).ToLocalChecked();
    for (uint32_t i = 0; i < names->Length(); i++) {
      auto name = names->Get(context, i).ToLocalChecked();
      v8::String::Utf8Value name_utf8val(isolate, name);
      const char* name_cstr = ToCString(name_utf8val);
      // TODO use format string.
      src.append("export const ");
      src.append(name_cstr);
      src.append(" = g.libdeno.builtinModules.");
      src.append(specifier_cstr);
      src.append(".");
      src.append(name_cstr);
      src.append(";\n");
    }
    auto export_str = v8_str(src.c_str(), true);

    auto module =
        CompileModule(context, specifier_cstr, export_str).ToLocalChecked();
    auto maybe_ok = module->InstantiateModule(context, ResolveCallback);
    CHECK(!maybe_ok.IsNothing());

    return handle_scope.Escape(module);
  }

  int ref_id = referrer->GetIdentityHash();
  auto range = d->module_info_map_.equal_range(ref_id);
  std::string referrer_filename;
  for (auto it = range.first; it != range.second; ++it) {
    // it->second: <string, v8::Persistent<v8::Module>>
    // operator== compares value identities stored in the handles
    // https://denolib.github.io/v8-docs/include_2v8_8h_source.html#l00487
    // Due to possibilities of identity hash collision, this is necessary
    if (it->second.second == referrer) {
      referrer_filename = it->second.first;
      break;
    }
  }
  CHECK(referrer_filename.size() != 0);

  v8::String::Utf8Value specifier_(isolate, specifier);
  const char* specifier_c = ToCString(specifier_);

  CHECK_NE(d->resolve_cb_, nullptr);
  d->resolve_cb_(d->user_data_, specifier_c, referrer_filename.c_str());

  if (d->resolve_module_.IsEmpty()) {
    // Resolution Error.
    std::stringstream err_ss;
    err_ss << "NotFound: Cannot resolve module \"" << specifier_c
           << "\" from \"" << referrer_filename << "\"";
    auto resolve_error = v8_str(err_ss.str().c_str());
    isolate->ThrowException(resolve_error);
    return v8::MaybeLocal<v8::Module>();
  } else {
    auto module = d->resolve_module_.Get(isolate);
    d->resolve_module_.Reset();
    return handle_scope.Escape(module);
  }
}

void DenoIsolate::ResolveOk(const char* filename, const char* source) {
  CHECK(resolve_module_.IsEmpty());
  auto count = module_map_.count(filename);
  if (count == 1) {
    auto module = module_map_[filename].Get(isolate_);
    resolve_module_.Reset(isolate_, module);
  } else {
    CHECK_EQ(count, 0);
    v8::HandleScope handle_scope(isolate_);
    auto context = context_.Get(isolate_);
    v8::TryCatch try_catch(isolate_);
    auto maybe_module = CompileModule(context, filename, v8_str(source, true));
    if (maybe_module.IsEmpty()) {
      DCHECK(try_catch.HasCaught());
      HandleException(context, try_catch.Exception());
    } else {
      auto module = maybe_module.ToLocalChecked();
      resolve_module_.Reset(isolate_, module);
    }
  }
}

bool ExecuteMod(v8::Local<v8::Context> context, const char* js_filename,
                const char* js_source, bool resolve_only) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto source = v8_str(js_source, true);

  v8::TryCatch try_catch(isolate);

  auto maybe_module = CompileModule(context, js_filename, source);

  if (maybe_module.IsEmpty()) {
    DCHECK(try_catch.HasCaught());
    HandleException(context, try_catch.Exception());
    return false;
  }
  DCHECK(!try_catch.HasCaught());

  auto module = maybe_module.ToLocalChecked();
  auto maybe_ok = module->InstantiateModule(context, ResolveCallback);
  if (maybe_ok.IsNothing()) {
    DCHECK(try_catch.HasCaught());
    HandleException(context, try_catch.Exception());
    return false;
  }

  CHECK_EQ(v8::Module::kInstantiated, module->GetStatus());

  if (resolve_only) {
    return true;
  }

  auto result = module->Evaluate(context);

  if (result.IsEmpty()) {
    DCHECK(try_catch.HasCaught());
    CHECK_EQ(v8::Module::kErrored, module->GetStatus());
    HandleException(context, module->GetException());
    return false;
  }

  return true;
}

bool Execute(v8::Local<v8::Context> context, const char* js_filename,
             const char* js_source) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto source = v8_str(js_source, true);
  auto name = v8_str(js_filename, true);

  v8::TryCatch try_catch(isolate);

  v8::ScriptOrigin origin(name);

  auto script = v8::Script::Compile(context, source, &origin);

  if (script.IsEmpty()) {
    DCHECK(try_catch.HasCaught());
    HandleException(context, try_catch.Exception());
    return false;
  }

  auto result = script.ToLocalChecked()->Run(context);

  if (result.IsEmpty()) {
    DCHECK(try_catch.HasCaught());
    HandleException(context, try_catch.Exception());
    return false;
  }

  return true;
}

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context) {
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto global = context->Global();

  auto deno_val = v8::Object::New(isolate);
  CHECK(global->Set(context, deno::v8_str("libdeno"), deno_val).FromJust());

  auto print_tmpl = v8::FunctionTemplate::New(isolate, Print);
  auto print_val = print_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(deno_val->Set(context, deno::v8_str("print"), print_val).FromJust());

  auto recv_tmpl = v8::FunctionTemplate::New(isolate, Recv);
  auto recv_val = recv_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(deno_val->Set(context, deno::v8_str("recv"), recv_val).FromJust());

  auto send_tmpl = v8::FunctionTemplate::New(isolate, Send);
  auto send_val = send_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(deno_val->Set(context, deno::v8_str("send"), send_val).FromJust());

  CHECK(deno_val->SetAccessor(context, deno::v8_str("shared"), Shared)
            .FromJust());

  CHECK(
      deno_val
          ->SetAccessor(context, deno::v8_str("builtinModules"), BuiltinModules)
          .FromJust());
}

void MessageCallback(v8::Local<v8::Message> message,
                     v8::Local<v8::Value> data) {
  auto* isolate = message->GetIsolate();
  DenoIsolate* d = static_cast<DenoIsolate*>(isolate->GetData(0));

  v8::HandleScope handle_scope(isolate);
  auto context = d->context_.Get(isolate);
  HandleExceptionMessage(context, message);
}

void DenoIsolate::AddIsolate(v8::Isolate* isolate) {
  isolate_ = isolate;
  // Leaving this code here because it will probably be useful later on, but
  // disabling it now as I haven't got tests for the desired behavior.
  // d->isolate->SetAbortOnUncaughtExceptionCallback(AbortOnUncaughtExceptionCallback);
  // d->isolate->AddMessageListener(MessageCallback2);
  // d->isolate->SetFatalErrorHandler(FatalErrorCallback2);
  isolate_->SetCaptureStackTraceForUncaughtExceptions(
      true, 10, v8::StackTrace::kDetailed);
  isolate_->SetPromiseRejectCallback(deno::PromiseRejectCallback);
  isolate_->SetData(0, this);
  isolate_->AddMessageListener(MessageCallback);
}

}  // namespace deno
