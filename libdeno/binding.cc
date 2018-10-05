// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <iostream>
#include <string>

#include "third_party/v8/include/libplatform/libplatform.h"
#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

#include "deno.h"
#include "internal.h"

namespace deno {

Deno* FromIsolate(v8::Isolate* isolate) {
  return static_cast<Deno*>(isolate->GetData(0));
}

void LazilyCreateDataMap(Deno* d) {
  if (d->async_data_map.IsEmpty()) {
    v8::HandleScope handle_scope(d->isolate);
    // It's important for security reasons that async_data_map is not exposed to
    // the VM.
    auto async_data_map = v8::Map::New(d->isolate);
    d->async_data_map.Reset(d->isolate, async_data_map);
  }
  DCHECK(!d->async_data_map.IsEmpty());
}

void AddDataRef(Deno* d, int32_t req_id, v8::Local<v8::Value> data_v) {
  LazilyCreateDataMap(d);
  auto async_data_map = d->async_data_map.Get(d->isolate);
  auto context = d->context.Get(d->isolate);
  auto req_id_v = v8::Integer::New(d->isolate, req_id);
  auto r = async_data_map->Set(context, req_id_v, data_v);
  CHECK(!r.IsEmpty());
}

void DeleteDataRef(Deno* d, int32_t req_id) {
  LazilyCreateDataMap(d);
  auto context = d->context.Get(d->isolate);
  // Delete persistent reference to data ArrayBuffer.
  auto async_data_map = d->async_data_map.Get(d->isolate);
  auto req_id_v = v8::Integer::New(d->isolate, req_id);
  auto maybe_deleted = async_data_map->Delete(context, req_id_v);
  CHECK(maybe_deleted.IsJust());
}

// Extracts a C string from a v8::V8 Utf8Value.
const char* ToCString(const v8::String::Utf8Value& value) {
  return *value ? *value : "<string conversion failed>";
}

static inline v8::Local<v8::String> v8_str(const char* x) {
  return v8::String::NewFromUtf8(v8::Isolate::GetCurrent(), x,
                                 v8::NewStringType::kNormal)
      .ToLocalChecked();
}

void HandleExceptionStr(v8::Local<v8::Context> context,
                        v8::Local<v8::Value> exception,
                        std::string* exception_str) {
  auto* isolate = context->GetIsolate();
  Deno* d = FromIsolate(isolate);

  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto message = v8::Exception::CreateMessage(isolate, exception);
  auto stack_trace = message->GetStackTrace();
  auto line =
      v8::Integer::New(isolate, message->GetLineNumber(context).FromJust());
  auto column =
      v8::Integer::New(isolate, message->GetStartColumn(context).FromJust());

  auto global_error_handler = d->global_error_handler.Get(isolate);

  if (!global_error_handler.IsEmpty()) {
    // global_error_handler is set so we try to handle the exception in
    // javascript.
    v8::Local<v8::Value> args[5];
    args[0] = exception->ToString(context).ToLocalChecked();
    args[1] = message->GetScriptResourceName();
    args[2] = line;
    args[3] = column;
    args[4] = exception;
    global_error_handler->Call(context->Global(), 5, args);
    /* message, source, lineno, colno, error */

    return;
  }

  char buf[12 * 1024];
  if (!stack_trace.IsEmpty()) {
    // No javascript error handler, but we do have a stack trace. Format it
    // into a string and add to last_exception.
    std::string msg;
    v8::String::Utf8Value exceptionStr(isolate, exception);
    msg += ToCString(exceptionStr);
    msg += "\n";

    for (int i = 0; i < stack_trace->GetFrameCount(); ++i) {
      auto frame = stack_trace->GetFrame(isolate, i);
      v8::String::Utf8Value script_name(isolate, frame->GetScriptName());
      int l = frame->GetLineNumber();
      int c = frame->GetColumn();
      snprintf(buf, sizeof(buf), "%s %d:%d\n", ToCString(script_name), l, c);
      msg += buf;
    }
    *exception_str += msg;
  } else {
    // No javascript error handler, no stack trace. Format the little info we
    // have into a string and add to last_exception.
    v8::String::Utf8Value exceptionStr(isolate, exception);
    v8::String::Utf8Value script_name(isolate,
                                      message->GetScriptResourceName());
    v8::String::Utf8Value line_str(isolate, line);
    v8::String::Utf8Value col_str(isolate, column);
    snprintf(buf, sizeof(buf), "%s\n%s %s:%s\n", ToCString(exceptionStr),
             ToCString(script_name), ToCString(line_str), ToCString(col_str));
    *exception_str += buf;
  }
}

void HandleException(v8::Local<v8::Context> context,
                     v8::Local<v8::Value> exception) {
  v8::Isolate* isolate = context->GetIsolate();
  Deno* d = FromIsolate(isolate);
  std::string exception_str;
  HandleExceptionStr(context, exception, &exception_str);
  if (d != nullptr) {
    d->last_exception = exception_str;
  } else {
    std::cerr << "Pre-Deno Exception " << exception_str << std::endl;
    exit(1);
  }
}

void ExitOnPromiseRejectCallback(
    v8::PromiseRejectMessage promise_reject_message) {
  auto* isolate = v8::Isolate::GetCurrent();
  Deno* d = static_cast<Deno*>(isolate->GetData(0));
  DCHECK_EQ(d->isolate, isolate);
  v8::HandleScope handle_scope(d->isolate);
  auto exception = promise_reject_message.GetValue();
  auto context = d->context.Get(d->isolate);
  HandleException(context, exception);
}

void Print(const v8::FunctionCallbackInfo<v8::Value>& args) {
  CHECK_GE(args.Length(), 1);
  CHECK_LE(args.Length(), 2);
  auto* isolate = args.GetIsolate();
  Deno* d = static_cast<Deno*>(isolate->GetData(0));
  auto context = d->context.Get(d->isolate);
  v8::HandleScope handle_scope(isolate);
  v8::String::Utf8Value str(isolate, args[0]);
  bool is_err =
      args.Length() >= 2 ? args[1]->BooleanValue(context).ToChecked() : false;
  const char* cstr = ToCString(str);
  auto& stream = is_err ? std::cerr : std::cout;
  stream << cstr << std::endl;
}

static v8::Local<v8::Uint8Array> ImportBuf(v8::Isolate* isolate, deno_buf buf) {
  if (buf.alloc_ptr == nullptr) {
    // If alloc_ptr isn't set, we memcpy.
    // This is currently used for flatbuffers created in Rust.
    auto ab = v8::ArrayBuffer::New(isolate, buf.data_len);
    memcpy(ab->GetContents().Data(), buf.data_ptr, buf.data_len);
    auto view = v8::Uint8Array::New(ab, 0, buf.data_len);
    return view;
  } else {
    auto ab = v8::ArrayBuffer::New(
        isolate, reinterpret_cast<void*>(buf.alloc_ptr), buf.alloc_len,
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

// Sets the recv callback.
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  Deno* d = reinterpret_cast<Deno*>(isolate->GetData(0));
  DCHECK_EQ(d->isolate, isolate);

  v8::HandleScope handle_scope(isolate);

  if (!d->recv.IsEmpty()) {
    isolate->ThrowException(v8_str("libdeno.recv already called."));
    return;
  }

  v8::Local<v8::Value> v = args[0];
  CHECK(v->IsFunction());
  v8::Local<v8::Function> func = v8::Local<v8::Function>::Cast(v);

  d->recv.Reset(isolate, func);
}

void Send(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  Deno* d = static_cast<Deno*>(isolate->GetData(0));
  DCHECK_EQ(d->isolate, isolate);

  v8::Locker locker(d->isolate);
  v8::EscapableHandleScope handle_scope(isolate);

  CHECK_EQ(d->currentArgs, nullptr);  // libdeno.send re-entry forbidden.
  int32_t req_id = d->next_req_id++;

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

  DCHECK_EQ(d->currentArgs, nullptr);
  d->currentArgs = &args;

  d->cb(d, req_id, control, data);

  if (d->currentArgs == nullptr) {
    // This indicates that deno_repond() was called already.
  } else {
    // Asynchronous.
    d->currentArgs = nullptr;
    // If the data ArrayBuffer was given, we must maintain a strong reference
    // to it until deno_respond is called.
    if (!data_v.IsEmpty()) {
      AddDataRef(d, req_id, data_v);
    }
  }
}

// Sets the global error handler.
void SetGlobalErrorHandler(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  Deno* d = reinterpret_cast<Deno*>(isolate->GetData(0));
  DCHECK_EQ(d->isolate, isolate);

  v8::HandleScope handle_scope(isolate);

  if (!d->global_error_handler.IsEmpty()) {
    isolate->ThrowException(
        v8_str("libdeno.setGlobalErrorHandler already called."));
    return;
  }

  v8::Local<v8::Value> v = args[0];
  CHECK(v->IsFunction());
  v8::Local<v8::Function> func = v8::Local<v8::Function>::Cast(v);

  d->global_error_handler.Reset(isolate, func);
}

bool ExecuteV8StringSource(v8::Local<v8::Context> context,
                           const char* js_filename,
                           v8::Local<v8::String> source) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);

  v8::Context::Scope context_scope(context);

  v8::TryCatch try_catch(isolate);

  auto name = v8_str(js_filename);

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

bool Execute(v8::Local<v8::Context> context, const char* js_filename,
             const char* js_source) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  auto source = v8_str(js_source);
  return ExecuteV8StringSource(context, js_filename, source);
}

void InitializeContext(v8::Isolate* isolate, v8::Local<v8::Context> context,
                       const char* js_filename, const std::string& js_source,
                       const std::string* source_map) {
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

  auto set_global_error_handler_tmpl =
      v8::FunctionTemplate::New(isolate, SetGlobalErrorHandler);
  auto set_global_error_handler_val =
      set_global_error_handler_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(deno_val
            ->Set(context, deno::v8_str("setGlobalErrorHandler"),
                  set_global_error_handler_val)
            .FromJust());

  {
    auto source = deno::v8_str(js_source.c_str());
    CHECK(
        deno_val->Set(context, deno::v8_str("mainSource"), source).FromJust());

    bool r = deno::ExecuteV8StringSource(context, js_filename, source);
    CHECK(r);

    if (source_map != nullptr) {
      CHECK_GT(source_map->length(), 1u);
      v8::TryCatch try_catch(isolate);
      v8::ScriptOrigin origin(v8_str("set_source_map.js"));
      std::string source_map_parens = "(" + *source_map + ")";
      auto source_map_v8_str = deno::v8_str(source_map_parens.c_str());
      auto script = v8::Script::Compile(context, source_map_v8_str, &origin);
      if (script.IsEmpty()) {
        DCHECK(try_catch.HasCaught());
        HandleException(context, try_catch.Exception());
        return;
      }
      auto source_map_obj = script.ToLocalChecked()->Run(context);
      if (source_map_obj.IsEmpty()) {
        DCHECK(try_catch.HasCaught());
        HandleException(context, try_catch.Exception());
        return;
      }
      CHECK(deno_val
                ->Set(context, deno::v8_str("mainSourceMap"),
                      source_map_obj.ToLocalChecked())
                .FromJust());
    }
  }
}

void AddIsolate(Deno* d, v8::Isolate* isolate) {
  d->next_req_id = 0;
  d->isolate = isolate;
  // Leaving this code here because it will probably be useful later on, but
  // disabling it now as I haven't got tests for the desired behavior.
  // d->isolate->SetCaptureStackTraceForUncaughtExceptions(true);
  // d->isolate->SetAbortOnUncaughtExceptionCallback(AbortOnUncaughtExceptionCallback);
  // d->isolate->AddMessageListener(MessageCallback2);
  // d->isolate->SetFatalErrorHandler(FatalErrorCallback2);
  d->isolate->SetPromiseRejectCallback(deno::ExitOnPromiseRejectCallback);
  d->isolate->SetData(0, d);
}

}  // namespace deno

extern "C" {

void deno_init() {
  // v8::V8::InitializeICUDefaultLocation(argv[0]);
  // v8::V8::InitializeExternalStartupData(argv[0]);
  auto* p = v8::platform::CreateDefaultPlatform();
  v8::V8::InitializePlatform(p);
  v8::V8::Initialize();
}

void* deno_get_data(Deno* d) { return d->data; }

const char* deno_v8_version() { return v8::V8::GetVersion(); }

void deno_set_v8_flags(int* argc, char** argv) {
  v8::V8::SetFlagsFromCommandLine(argc, argv, true);
}

const char* deno_last_exception(Deno* d) { return d->last_exception.c_str(); }

int deno_execute(Deno* d, const char* js_filename, const char* js_source) {
  auto* isolate = d->isolate;
  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  auto context = d->context.Get(d->isolate);
  return deno::Execute(context, js_filename, js_source) ? 1 : 0;
}

int deno_respond(Deno* d, int32_t req_id, deno_buf buf) {
  if (d->currentArgs != nullptr) {
    // Synchronous response.
    auto ab = deno::ImportBuf(d->isolate, buf);
    d->currentArgs->GetReturnValue().Set(ab);
    d->currentArgs = nullptr;
    return 0;
  }

  // Asynchronous response.

  v8::Locker locker(d->isolate);
  v8::Isolate::Scope isolate_scope(d->isolate);
  v8::HandleScope handle_scope(d->isolate);

  auto context = d->context.Get(d->isolate);
  v8::Context::Scope context_scope(context);

  v8::TryCatch try_catch(d->isolate);

  deno::DeleteDataRef(d, req_id);

  auto recv = d->recv.Get(d->isolate);
  if (recv.IsEmpty()) {
    d->last_exception = "libdeno.recv has not been called.";
    return 1;
  }

  v8::Local<v8::Value> args[1];
  args[0] = deno::ImportBuf(d->isolate, buf);
  recv->Call(context->Global(), 1, args);

  if (try_catch.HasCaught()) {
    deno::HandleException(context, try_catch.Exception());
    return 1;
  }

  return 0;
}

void deno_delete(Deno* d) {
  d->isolate->Dispose();
  delete d;
}

void deno_terminate_execution(Deno* d) { d->isolate->TerminateExecution(); }

}  // extern "C"
