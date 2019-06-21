// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <algorithm>
#include <iostream>
#include <string>

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <io.h>
#include <windows.h>
#endif  // _WIN32

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
  DCHECK_NULL(data);
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
  DCHECK_NULL(data);
  InternalFieldData* embedder_field = static_cast<InternalFieldData*>(
      holder->GetAlignedPointerFromInternalField(index));
  if (embedder_field == nullptr) return {nullptr, 0};
  int size = sizeof(*embedder_field);
  char* payload = new char[size];
  // We simply use memcpy to serialize the content.
  memcpy(payload, embedder_field, size);
  return {payload, size};
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
  auto* isolate = args.GetIsolate();
  int argsLen = args.Length();
  if (argsLen < 1 || argsLen > 2) {
    ThrowInvalidArgument(isolate);
  }
  v8::HandleScope handle_scope(isolate);
  bool is_err = args.Length() >= 2 ? args[1]->BooleanValue(isolate) : false;
  FILE* file = is_err ? stderr : stdout;

#ifdef _WIN32
  int fd = _fileno(file);
  if (fd < 0) return;

  HANDLE h = reinterpret_cast<HANDLE>(_get_osfhandle(fd));
  if (h == INVALID_HANDLE_VALUE) return;

  DWORD mode;
  if (GetConsoleMode(h, &mode)) {
    // Print to Windows console. Since the Windows API generally doesn't support
    // UTF-8 encoded text, we have to use `WriteConsoleW()` which uses UTF-16.
    v8::String::Value str(isolate, args[0]);
    auto str_len = static_cast<size_t>(str.length());
    auto str_wchars = reinterpret_cast<WCHAR*>(*str);

    // WriteConsoleW has some limit to how many characters can be written at
    // once, which is unspecified but low enough to be encountered in practice.
    // Therefore we break up the write into chunks of 8kb if necessary.
    size_t chunk_start = 0;
    while (chunk_start < str_len) {
      size_t chunk_end = std::min(chunk_start + 8192, str_len);

      // Do not break in the middle of a surrogate pair. Note that `chunk_end`
      // points to the start of the next chunk, so we check whether it contains
      // the second half of a surrogate pair (a.k.a. "low surrogate").
      if (chunk_end < str_len && str_wchars[chunk_end] >= 0xdc00 &&
          str_wchars[chunk_end] <= 0xdfff) {
        --chunk_end;
      }

      // Write to the console.
      DWORD chunk_len = static_cast<DWORD>(chunk_end - chunk_start);
      DWORD _;
      WriteConsoleW(h, &str_wchars[chunk_start], chunk_len, &_, nullptr);

      chunk_start = chunk_end;
    }
    return;
  }
#endif  // _WIN32

  v8::String::Utf8Value str(isolate, args[0]);
  fwrite(*str, sizeof(**str), str.length(), file);
  fflush(file);
}

void ErrorToJSON(const v8::FunctionCallbackInfo<v8::Value>& args) {
  CHECK_EQ(args.Length(), 1);
  auto* isolate = args.GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  auto context = d->context_.Get(d->isolate_);
  v8::HandleScope handle_scope(isolate);
  auto json_string = EncodeExceptionAsJSON(context, args[0]);
  args.GetReturnValue().Set(v8_str(json_string.c_str()));
}

v8::Local<v8::Uint8Array> ImportBuf(DenoIsolate* d, deno_buf buf) {
  if (buf.data_ptr == nullptr) {
    return v8::Local<v8::Uint8Array>();
  }

  // To avoid excessively allocating new ArrayBuffers, we try to reuse a single
  // global ArrayBuffer. The caveat is that users must extract data from it
  // before the next tick. We only do this for ArrayBuffers less than 1024
  // bytes.
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
      DCHECK_NULL(d->global_import_buf_ptr_);
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
}

// Sets the recv_ callback.
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  DCHECK_EQ(d->isolate_, isolate);

  v8::HandleScope handle_scope(isolate);

  if (!d->recv_.IsEmpty()) {
    isolate->ThrowException(v8_str("Deno.core.recv already called."));
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

  v8::HandleScope handle_scope(isolate);

  deno_buf control = {nullptr, 0};
  if (args[0]->IsArrayBufferView()) {
    auto view = v8::Local<v8::ArrayBufferView>::Cast(args[0]);
    auto data =
        reinterpret_cast<uint8_t*>(view->Buffer()->GetContents().Data());
    control = {data + view->ByteOffset(), view->ByteLength()};
  }

  PinnedBuf zero_copy =
      args[1]->IsArrayBufferView()
          ? PinnedBuf(v8::Local<v8::ArrayBufferView>::Cast(args[1]))
          : PinnedBuf();

  DCHECK_NULL(d->current_args_);
  d->current_args_ = &args;

  d->recv_cb_(d->user_data_, control, zero_copy.IntoRaw());

  if (d->current_args_ == nullptr) {
    // This indicates that deno_repond() was called already.
  } else {
    // Asynchronous.
    d->current_args_ = nullptr;
  }
}

v8::ScriptOrigin ModuleOrigin(v8::Isolate* isolate,
                              v8::Local<v8::Value> resource_name) {
  return v8::ScriptOrigin(resource_name, v8::Local<v8::Integer>(),
                          v8::Local<v8::Integer>(), v8::Local<v8::Boolean>(),
                          v8::Local<v8::Integer>(), v8::Local<v8::Value>(),
                          v8::Local<v8::Boolean>(), v8::Local<v8::Boolean>(),
                          v8::True(isolate));
}

deno_mod DenoIsolate::RegisterModule(bool main, const char* name,
                                     const char* source) {
  v8::Isolate::Scope isolate_scope(isolate_);
  v8::Locker locker(isolate_);
  v8::HandleScope handle_scope(isolate_);
  auto context = context_.Get(isolate_);
  v8::Context::Scope context_scope(context);

  v8::Local<v8::String> name_str = v8_str(name);
  v8::Local<v8::String> source_str = v8_str(source);

  auto origin = ModuleOrigin(isolate_, name_str);
  v8::ScriptCompiler::Source source_(source_str, origin);

  v8::TryCatch try_catch(isolate_);

  auto maybe_module = v8::ScriptCompiler::CompileModule(isolate_, &source_);

  if (try_catch.HasCaught()) {
    CHECK(maybe_module.IsEmpty());
    HandleException(context, try_catch.Exception());
    return 0;
  }

  auto module = maybe_module.ToLocalChecked();

  int id = module->GetIdentityHash();

  std::vector<std::string> import_specifiers;

  for (int i = 0; i < module->GetModuleRequestsLength(); ++i) {
    v8::Local<v8::String> specifier = module->GetModuleRequest(i);
    v8::String::Utf8Value specifier_utf8(isolate_, specifier);
    import_specifiers.push_back(*specifier_utf8);
  }

  mods_.emplace(
      std::piecewise_construct, std::make_tuple(id),
      std::make_tuple(isolate_, module, main, name, import_specifiers));
  mods_by_name_[name] = id;

  return id;
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
  v8::Local<v8::SharedArrayBuffer> ab;
  if (d->shared_ab_.IsEmpty()) {
    // Lazily initialize the persistent external ArrayBuffer.
    ab = v8::SharedArrayBuffer::New(isolate, d->shared_.data_ptr,
                                    d->shared_.data_len,
                                    v8::ArrayBufferCreationMode::kExternalized);
    d->shared_ab_.Reset(isolate, ab);
  }
  auto shared_ab = d->shared_ab_.Get(isolate);
  info.GetReturnValue().Set(shared_ab);
}

void DenoIsolate::ClearModules() {
  for (auto it = mods_.begin(); it != mods_.end(); it++) {
    it->second.handle.Reset();
  }
  mods_.clear();
  mods_by_name_.clear();
}

bool Execute(v8::Local<v8::Context> context, const char* js_filename,
             const char* js_source) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto source = v8_str(js_source);
  auto name = v8_str(js_filename);

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

static inline v8::Local<v8::Boolean> v8_bool(bool v) {
  return v8::Boolean::New(v8::Isolate::GetCurrent(), v);
}

void EvalContext(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  v8::EscapableHandleScope handleScope(isolate);
  auto context = d->context_.Get(isolate);
  v8::Context::Scope context_scope(context);

  if (!(args[0]->IsString())) {
    ThrowInvalidArgument(isolate);
    return;
  }

  auto source = args[0].As<v8::String>();

  auto output = v8::Array::New(isolate, 2);
  /**
   * output[0] = result
   * output[1] = ErrorInfo | null
   *   ErrorInfo = {
   *     thrown: Error | any,
   *     isNativeError: boolean,
   *     isCompileError: boolean,
   *   }
   */

  v8::TryCatch try_catch(isolate);

  auto name = v8_str("<unknown>");
  v8::ScriptOrigin origin(name);
  auto script = v8::Script::Compile(context, source, &origin);

  if (script.IsEmpty()) {
    DCHECK(try_catch.HasCaught());
    auto exception = try_catch.Exception();

    output->Set(0, v8::Null(isolate));

    auto errinfo_obj = v8::Object::New(isolate);
    errinfo_obj->Set(v8_str("isCompileError"), v8_bool(true));
    errinfo_obj->Set(v8_str("isNativeError"),
                     v8_bool(exception->IsNativeError()));
    errinfo_obj->Set(v8_str("thrown"), exception);

    output->Set(1, errinfo_obj);

    args.GetReturnValue().Set(output);
    return;
  }

  auto result = script.ToLocalChecked()->Run(context);

  if (result.IsEmpty()) {
    DCHECK(try_catch.HasCaught());
    auto exception = try_catch.Exception();

    output->Set(0, v8::Null(isolate));

    auto errinfo_obj = v8::Object::New(isolate);
    errinfo_obj->Set(v8_str("isCompileError"), v8_bool(false));
    errinfo_obj->Set(v8_str("isNativeError"),
                     v8_bool(exception->IsNativeError()));
    errinfo_obj->Set(v8_str("thrown"), exception);

    output->Set(1, errinfo_obj);

    args.GetReturnValue().Set(output);
    return;
  }

  output->Set(0, result.ToLocalChecked());
  output->Set(1, v8::Null(isolate));
  args.GetReturnValue().Set(output);
}

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context) {
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto global = context->Global();

  auto deno_val = v8::Object::New(isolate);
  CHECK(global->Set(context, deno::v8_str("Deno"), deno_val).FromJust());

  auto core_val = v8::Object::New(isolate);
  CHECK(deno_val->Set(context, deno::v8_str("core"), core_val).FromJust());

  auto print_tmpl = v8::FunctionTemplate::New(isolate, Print);
  auto print_val = print_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("print"), print_val).FromJust());

  auto recv_tmpl = v8::FunctionTemplate::New(isolate, Recv);
  auto recv_val = recv_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("recv"), recv_val).FromJust());

  auto send_tmpl = v8::FunctionTemplate::New(isolate, Send);
  auto send_val = send_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("send"), send_val).FromJust());

  auto eval_context_tmpl = v8::FunctionTemplate::New(isolate, EvalContext);
  auto eval_context_val =
      eval_context_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("evalContext"), eval_context_val)
            .FromJust());

  auto error_to_json_tmpl = v8::FunctionTemplate::New(isolate, ErrorToJSON);
  auto error_to_json_val =
      error_to_json_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("errorToJSON"), error_to_json_val)
            .FromJust());

  CHECK(core_val->SetAccessor(context, deno::v8_str("shared"), Shared)
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

void HostInitializeImportMetaObjectCallback(v8::Local<v8::Context> context,
                                            v8::Local<v8::Module> module,
                                            v8::Local<v8::Object> meta) {
  auto* isolate = context->GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  v8::Isolate::Scope isolate_scope(isolate);

  CHECK(!module.IsEmpty());

  deno_mod id = module->GetIdentityHash();
  CHECK_NE(id, 0);

  auto* info = d->GetModuleInfo(id);

  const char* url = info->name.c_str();
  const bool main = info->main;

  meta->CreateDataProperty(context, v8_str("url"), v8_str(url)).ToChecked();
  meta->CreateDataProperty(context, v8_str("main"), v8_bool(main)).ToChecked();
}

v8::MaybeLocal<v8::Promise> HostImportModuleDynamicallyCallback(
    v8::Local<v8::Context> context, v8::Local<v8::ScriptOrModule> referrer,
    v8::Local<v8::String> specifier) {
  auto* isolate = context->GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  v8::Context::Scope context_scope(context);
  v8::EscapableHandleScope handle_scope(isolate);

  v8::String::Utf8Value specifier_str(isolate, specifier);

  auto referrer_name = referrer->GetResourceName();
  v8::String::Utf8Value referrer_name_str(isolate, referrer_name);

  // TODO(ry) I'm not sure what HostDefinedOptions is for or if we're ever going
  // to use it. For now we check that it is not used. This check may need to be
  // changed in the future.
  auto host_defined_options = referrer->GetHostDefinedOptions();
  CHECK_EQ(host_defined_options->Length(), 0);

  v8::Local<v8::Promise::Resolver> resolver =
      v8::Promise::Resolver::New(context).ToLocalChecked();

  deno_dyn_import_id import_id = d->next_dyn_import_id_++;

  d->dyn_import_map_.emplace(std::piecewise_construct,
                             std::make_tuple(import_id),
                             std::make_tuple(d->isolate_, resolver));

  d->dyn_import_cb_(d->user_data_, *specifier_str, *referrer_name_str,
                    import_id);

  auto promise = resolver->GetPromise();
  return handle_scope.Escape(promise);
}

void DenoIsolate::AddIsolate(v8::Isolate* isolate) {
  isolate_ = isolate;
  isolate_->SetCaptureStackTraceForUncaughtExceptions(
      true, 10, v8::StackTrace::kDetailed);
  isolate_->SetPromiseRejectCallback(deno::PromiseRejectCallback);
  isolate_->SetData(0, this);
  isolate_->AddMessageListener(MessageCallback);
  isolate->SetHostInitializeImportMetaObjectCallback(
      HostInitializeImportMetaObjectCallback);
  isolate->SetHostImportModuleDynamicallyCallback(
      HostImportModuleDynamicallyCallback);
}

}  // namespace deno
