// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "exceptions.h"
#include "internal.h"

using deno::DenoIsolate;
using deno::HandleException;
using v8::Boolean;
using v8::Context;
using v8::EscapableHandleScope;
using v8::HandleScope;
using v8::Integer;
using v8::Isolate;
using v8::Local;
using v8::Locker;
using v8::Module;
using v8::Object;
using v8::ScriptCompiler;
using v8::ScriptOrigin;
using v8::String;
using v8::Value;

v8::MaybeLocal<v8::Module> ResolveCallback(Local<Context> context,
                                           Local<String> specifier,
                                           Local<Module> referrer) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::Locker locker(isolate);

  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);

  v8::EscapableHandleScope handle_scope(isolate);

  deno_mod referrer_id = referrer->GetIdentityHash();
  auto* referrer_info = d->GetModuleInfo(referrer_id);
  CHECK_NOT_NULL(referrer_info);

  for (int i = 0; i < referrer->GetModuleRequestsLength(); i++) {
    Local<String> req = referrer->GetModuleRequest(i);

    if (req->Equals(context, specifier).ToChecked()) {
      v8::String::Utf8Value req_utf8(isolate, req);
      std::string req_str(*req_utf8);

      deno_mod id = d->resolve_cb_(d->user_data_, req_str.c_str(), referrer_id);

      // Note: id might be zero, in which case GetModuleInfo will return
      // nullptr.
      auto* info = d->GetModuleInfo(id);
      if (info == nullptr) {
        char buf[64 * 1024];
        snprintf(buf, sizeof(buf), "Cannot resolve module \"%s\" from \"%s\"",
                 req_str.c_str(), referrer_info->name.c_str());
        isolate->ThrowException(deno::v8_str(buf));
        break;
      } else {
        Local<Module> child_mod = info->handle.Get(isolate);
        return handle_scope.Escape(child_mod);
      }
    }
  }

  return v8::MaybeLocal<v8::Module>();  // Error
}

extern "C" {

deno_mod deno_mod_new(Deno* d_, bool main, const char* name_cstr,
                      const char* source_cstr) {
  auto* d = unwrap(d_);
  return d->RegisterModule(main, name_cstr, source_cstr);
}

const char* deno_mod_name(Deno* d_, deno_mod id) {
  auto* d = unwrap(d_);
  auto* info = d->GetModuleInfo(id);
  return info->name.c_str();
}

size_t deno_mod_imports_len(Deno* d_, deno_mod id) {
  auto* d = unwrap(d_);
  auto* info = d->GetModuleInfo(id);
  return info->import_specifiers.size();
}

const char* deno_mod_imports_get(Deno* d_, deno_mod id, size_t index) {
  auto* d = unwrap(d_);
  auto* info = d->GetModuleInfo(id);
  if (info == nullptr || index >= info->import_specifiers.size()) {
    return nullptr;
  } else {
    return info->import_specifiers[index].c_str();
  }
}

void deno_mod_instantiate(Deno* d_, void* user_data, deno_mod id,
                          deno_resolve_cb cb) {
  auto* d = unwrap(d_);
  deno::UserDataScope user_data_scope(d, user_data);

  auto* isolate = d->isolate_;
  v8::Isolate::Scope isolate_scope(isolate);
  v8::Locker locker(isolate);
  v8::HandleScope handle_scope(isolate);
  auto context = d->context_.Get(d->isolate_);
  v8::Context::Scope context_scope(context);

  v8::TryCatch try_catch(isolate);
  {
    CHECK_NULL(d->resolve_cb_);
    d->resolve_cb_ = cb;
    {
      auto* info = d->GetModuleInfo(id);
      if (info == nullptr) {
        return;
      }
      Local<Module> module = info->handle.Get(isolate);
      if (module->GetStatus() == Module::kErrored) {
        return;
      }
      auto maybe_ok = module->InstantiateModule(context, ResolveCallback);
      CHECK(maybe_ok.IsJust() || try_catch.HasCaught());
    }
    d->resolve_cb_ = nullptr;
  }

  if (try_catch.HasCaught()) {
    HandleException(context, try_catch.Exception());
  }
}

void deno_mod_evaluate(Deno* d_, void* user_data, deno_mod id) {
  auto* d = unwrap(d_);
  deno::UserDataScope user_data_scope(d, user_data);

  auto* isolate = d->isolate_;
  v8::Isolate::Scope isolate_scope(isolate);
  v8::Locker locker(isolate);
  v8::HandleScope handle_scope(isolate);
  auto context = d->context_.Get(d->isolate_);
  v8::Context::Scope context_scope(context);

  auto* info = d->GetModuleInfo(id);
  Local<Module> module = info->handle.Get(isolate);

  CHECK_EQ(Module::kInstantiated, module->GetStatus());

  auto maybe_result = module->Evaluate(context);
  if (maybe_result.IsEmpty()) {
    CHECK_EQ(Module::kErrored, module->GetStatus());
    HandleException(context, module->GetException());
  }
}

void deno_dyn_import(Deno* d_, void* user_data, deno_dyn_import_id import_id,
                     deno_mod mod_id) {
  auto* d = unwrap(d_);
  deno::UserDataScope user_data_scope(d, user_data);

  auto* isolate = d->isolate_;
  v8::Isolate::Scope isolate_scope(isolate);
  v8::Locker locker(isolate);
  v8::HandleScope handle_scope(isolate);
  auto context = d->context_.Get(d->isolate_);
  v8::Context::Scope context_scope(context);

  v8::TryCatch try_catch(isolate);

  auto it = d->dyn_import_map_.find(import_id);
  if (it == d->dyn_import_map_.end()) {
    CHECK(false);  // TODO(ry) error on bad import_id.
    return;
  }

  /// Resolve.
  auto persistent_promise = &it->second;
  auto promise = persistent_promise->Get(isolate);

  auto* info = d->GetModuleInfo(mod_id);

  // Do the following callback into JS?? Is user_data_scope needed?
  persistent_promise->Reset();
  d->dyn_import_map_.erase(it);

  if (info == nullptr) {
    // Resolution error.
    promise->Reject(context, v8::Null(isolate)).ToChecked();
  } else {
    // Resolution success
    Local<Module> module = info->handle.Get(isolate);
    CHECK_GE(module->GetStatus(), v8::Module::kInstantiated);
    Local<Value> module_namespace = module->GetModuleNamespace();
    promise->Resolve(context, module_namespace).ToChecked();
  }

  if (try_catch.HasCaught()) {
    HandleException(context, try_catch.Exception());
  }
}

}  // extern "C"
