/*
Copyright 2018 Ryan Dahl <ry@tinyclouds.org>. All rights reserved.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to
deal in the Software without restriction, including without limitation the
rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
IN THE SOFTWARE.
*/
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>

#include "v8/include/libplatform/libplatform.h"
#include "v8/include/v8.h"

#include "./deno_internal.h"
#include "include/deno.h"

#define CHECK(x) assert(x)  // TODO(ry) use V8's CHECK.

namespace deno {

// Extracts a C string from a v8::V8 Utf8Value.
const char* ToCString(const v8::String::Utf8Value& value) {
  return *value ? *value : "<string conversion failed>";
}

static inline v8::Local<v8::String> v8_str(const char* x) {
  return v8::String::NewFromUtf8(v8::Isolate::GetCurrent(), x,
                                 v8::NewStringType::kNormal)
      .ToLocalChecked();
}

void HandleException(v8::Local<v8::Context> context,
                     v8::Local<v8::Value> exception) {
  auto* isolate = context->GetIsolate();
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto message = v8::Exception::CreateMessage(isolate, exception);
  auto onerrorStr = v8::String::NewFromUtf8(isolate, "onerror");
  auto onerror = context->Global()->Get(onerrorStr);

  if (onerror->IsFunction()) {
    auto func = v8::Local<v8::Function>::Cast(onerror);
    v8::Local<v8::Value> args[5];
    auto origin = message->GetScriptOrigin();
    args[0] = exception->ToString();
    args[1] = message->GetScriptResourceName();
    args[2] = origin.ResourceLineOffset();
    args[3] = origin.ResourceColumnOffset();
    args[4] = exception;
    func->Call(context->Global(), 5, args);
    /* message, source, lineno, colno, error */
  } else {
    v8::String::Utf8Value exceptionStr(isolate, exception);
    printf("Unhandled Exception %s\n", ToCString(exceptionStr));
    message->PrintCurrentStackTrace(isolate, stdout);
  }
}

/*
bool AbortOnUncaughtExceptionCallback(v8::Isolate* isolate) {
  return true;
}

void MessageCallback2(Local<Message> message, v8::Local<v8::Value> data) {
  printf("MessageCallback2\n\n");
}

void FatalErrorCallback2(const char* location, const char* message) {
  printf("FatalErrorCallback2\n");
}
*/

void ExitOnPromiseRejectCallback(
    v8::PromiseRejectMessage promise_reject_message) {
  auto* isolate = v8::Isolate::GetCurrent();
  Deno* d = static_cast<Deno*>(isolate->GetData(0));
  assert(d->isolate == isolate);
  v8::HandleScope handle_scope(d->isolate);
  auto exception = promise_reject_message.GetValue();
  auto context = d->context.Get(d->isolate);
  HandleException(context, exception);
}

void Print(const v8::FunctionCallbackInfo<v8::Value>& args) {
  assert(args.Length() == 1);
  auto* isolate = args.GetIsolate();
  v8::HandleScope handle_scope(isolate);
  v8::String::Utf8Value str(isolate, args[0]);
  const char* cstr = ToCString(str);
  printf("%s\n", cstr);
  fflush(stdout);
}

// Sets the recv callback.
void Recv(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  Deno* d = reinterpret_cast<Deno*>(isolate->GetData(0));
  assert(d->isolate == isolate);

  v8::HandleScope handle_scope(isolate);

  v8::Local<v8::Value> v = args[0];
  assert(v->IsFunction());
  v8::Local<v8::Function> func = v8::Local<v8::Function>::Cast(v);

  d->recv.Reset(isolate, func);
}

// Called from JavaScript, routes message to golang.
void Send(const v8::FunctionCallbackInfo<v8::Value>& args) {
  v8::Isolate* isolate = args.GetIsolate();
  Deno* d = static_cast<Deno*>(isolate->GetData(0));
  assert(d->isolate == isolate);

  v8::Locker locker(d->isolate);
  v8::EscapableHandleScope handle_scope(isolate);

  v8::Local<v8::Value> v = args[0];
  assert(v->IsArrayBuffer());

  auto ab = v8::Local<v8::ArrayBuffer>::Cast(v);
  auto contents = ab->GetContents();

  void* buf = contents.Data();
  int buflen = static_cast<int>(contents.ByteLength());

  auto retbuf = d->cb(d, deno_buf{buf, buflen});
  if (retbuf.data) {
    auto ab = v8::ArrayBuffer::New(d->isolate, retbuf.data, retbuf.len,
                                   v8::ArrayBufferCreationMode::kInternalized);
    /*
    // I'm slightly worried the above v8::ArrayBuffer construction leaks memory
    // the following might be a safer way to do it.
    auto ab = v8::ArrayBuffer::New(d->isolate, retbuf.len);
    auto contents = ab->GetContents();
    memcpy(contents.Data(), retbuf.data, retbuf.len);
    free(retbuf.data);
    */
    args.GetReturnValue().Set(handle_scope.Escape(ab));
  }
}

bool Load(v8::Local<v8::Context> context, const char* name_s,
          const char* source_s) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);

  v8::Context::Scope context_scope(context);

  v8::TryCatch try_catch(isolate);

  auto name = v8_str(name_s);
  auto source = v8_str(source_s);

  v8::ScriptOrigin origin(name);

  auto script = v8::Script::Compile(context, source, &origin);

  if (script.IsEmpty()) {
    assert(try_catch.HasCaught());
    HandleException(context, try_catch.Exception());
    return false;
  }

  auto result = script.ToLocalChecked()->Run(context);

  if (result.IsEmpty()) {
    assert(try_catch.HasCaught());
    HandleException(context, try_catch.Exception());
    return false;
  }

  return true;
}

v8::StartupData MakeSnapshot(v8::StartupData* prev_natives_blob,
                             v8::StartupData* prev_snapshot_blob,
                             const char* js_filename, const char* js_source) {
  v8::V8::SetNativesDataBlob(prev_natives_blob);
  v8::V8::SetSnapshotDataBlob(prev_snapshot_blob);

  auto* creator = new v8::SnapshotCreator(external_references);
  auto* isolate = creator->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  {
    v8::HandleScope handle_scope(isolate);
    auto context = v8::Context::New(isolate);
    v8::Context::Scope context_scope(context);

    auto global = context->Global();

    auto print_tmpl = v8::FunctionTemplate::New(isolate, Print);
    auto print_val = print_tmpl->GetFunction(context).ToLocalChecked();
    CHECK(
        global->Set(context, deno::v8_str("deno_print"), print_val).FromJust());

    auto recv_tmpl = v8::FunctionTemplate::New(isolate, Recv);
    auto recv_val = recv_tmpl->GetFunction(context).ToLocalChecked();
    CHECK(global->Set(context, deno::v8_str("deno_recv"), recv_val).FromJust());

    auto send_tmpl = v8::FunctionTemplate::New(isolate, Send);
    auto send_val = send_tmpl->GetFunction(context).ToLocalChecked();
    CHECK(global->Set(context, deno::v8_str("deno_send"), send_val).FromJust());

    bool r = Load(context, js_filename, js_source);
    assert(r);

    creator->SetDefaultContext(context);
  }

  auto snapshot_blob =
      creator->CreateBlob(v8::SnapshotCreator::FunctionCodeHandling::kKeep);

  return snapshot_blob;
}

void AddIsolate(Deno* d, v8::Isolate* isolate) {
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

const char* deno_v8_version() { return v8::V8::GetVersion(); }

void deno_set_flags(int* argc, char** argv) {
  v8::V8::SetFlagsFromCommandLine(argc, argv, true);
}

const char* deno_last_exception(Deno* d) { return d->last_exception.c_str(); }

bool deno_load(Deno* d, const char* name_s, const char* source_s) {
  auto* isolate = d->isolate;
  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  auto context = d->context.Get(d->isolate);
  return deno::Load(context, name_s, source_s);
}

// Routes message to the javascript callback set with deno_recv().
// False return value indicates error. Check deno_last_exception() for exception
// text.
bool deno_send(Deno* d, deno_buf buf) {
  v8::Locker locker(d->isolate);
  v8::Isolate::Scope isolate_scope(d->isolate);
  v8::HandleScope handle_scope(d->isolate);

  auto context = d->context.Get(d->isolate);
  v8::Context::Scope context_scope(context);

  v8::TryCatch try_catch(d->isolate);

  v8::Local<v8::Function> recv =
      v8::Local<v8::Function>::New(d->isolate, d->recv);
  if (recv.IsEmpty()) {
    d->last_exception = "deno_recv has not been called.";
    return false;
  }

  v8::Local<v8::Value> args[1];
  args[0] = v8::ArrayBuffer::New(d->isolate, buf.data, buf.len,
                                 v8::ArrayBufferCreationMode::kInternalized);
  assert(!args[0].IsEmpty());
  assert(!try_catch.HasCaught());

  recv->Call(context->Global(), 1, args);

  if (try_catch.HasCaught()) {
    deno::HandleException(context, try_catch.Exception());
    return false;
  }

  return true;
}

void deno_dispose(Deno* d) {
  d->isolate->Dispose();
  delete (d);
}

void deno_terminate_execution(Deno* d) { d->isolate->TerminateExecution(); }

}  // extern "C"
