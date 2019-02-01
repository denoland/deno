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

std::string BuiltinModuleSrc(Local<Context> context, Local<String> specifier) {
  auto* isolate = context->GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  v8::EscapableHandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  v8::String::Utf8Value specifier_utf8val(isolate, specifier);
  const char* specifier_cstr = *specifier_utf8val;

  auto builtin_modules = d->GetBuiltinModules();
  auto val = builtin_modules->Get(context, specifier).ToLocalChecked();
  CHECK(val->IsObject());
  auto obj = val->ToObject(isolate);

  // In order to export obj as a module, we must iterate over its properties
  // and export them each individually.
  // TODO(ry) Find a better way to do this.
  std::string src = "let globalEval = eval\nlet g = globalEval('this');\n";
  auto names = obj->GetOwnPropertyNames(context).ToLocalChecked();
  for (uint32_t i = 0; i < names->Length(); i++) {
    auto name = names->Get(context, i).ToLocalChecked();
    v8::String::Utf8Value name_utf8val(isolate, name);
    const char* name_cstr = *name_utf8val;
    // TODO(ry) use format string.
    src.append("export const ");
    src.append(name_cstr);
    src.append(" = g.libdeno.builtinModules.");
    src.append(specifier_cstr);
    src.append(".");
    src.append(name_cstr);
    src.append(";\n");
  }
  return src;
}

v8::MaybeLocal<v8::Module> ResolveCallback(Local<Context> context,
                                           Local<String> specifier,
                                           Local<Module> referrer) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::Locker locker(isolate);

  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);

  v8::EscapableHandleScope handle_scope(isolate);

  auto builtin_modules = d->GetBuiltinModules();

  deno_mod referrer_id = referrer->GetIdentityHash();
  auto* referrer_info = d->GetModuleInfo(referrer_id);
  CHECK_NE(referrer_info, nullptr);

  for (int i = 0; i < referrer->GetModuleRequestsLength(); i++) {
    Local<String> req = referrer->GetModuleRequest(i);

    if (req->Equals(context, specifier).ToChecked()) {
      v8::String::Utf8Value req_utf8(isolate, req);
      std::string req_str(*req_utf8);

      deno_mod id = 0;
      {
        bool has_builtin = builtin_modules->Has(context, specifier).ToChecked();
        if (has_builtin) {
          auto it = d->mods_by_name_.find(req_str.c_str());
          if (it != d->mods_by_name_.end()) {
            id = it->second;
          } else {
            std::string src = BuiltinModuleSrc(context, specifier);
            id = d->RegisterModule(req_str.c_str(), src.c_str());
          }
        } else {
          id = d->resolve_cb_(d->user_data_, req_str.c_str(), referrer_id);
        }
      }

      // Note: id might be zero, in which case GetModuleInfo will return
      // nullptr.
      auto* info = d->GetModuleInfo(id);
      if (info == nullptr) {
        char buf[64 * 1024];
        snprintf(buf, sizeof(buf), "Cannot resolve module \"%s\" from \"%s\"",
                 req_str.c_str(), referrer_info->name.c_str());
        isolate->ThrowException(deno::v8_str(buf, true));
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

deno_mod deno_mod_new(Deno* d_, const char* name_cstr,
                      const char* source_cstr) {
  auto* d = unwrap(d_);
  return d->RegisterModule(name_cstr, source_cstr);
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
    CHECK_EQ(nullptr, d->resolve_cb_);
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

}  // extern "C"
