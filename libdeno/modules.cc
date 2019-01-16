#include "modules.h"
#include "exceptions.h"

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

namespace deno {

v8::Local<v8::Object> GetBuiltinModules(v8::Local<v8::Context> context) {
  v8::EscapableHandleScope handle_scope(context->GetIsolate());
  v8::Local<v8::Object> builtin_modules =
      context->GetEmbedderData(kBuiltinModules)
          ->ToObject(context)
          .ToLocalChecked();
  return handle_scope.Escape(builtin_modules);
}

v8::ScriptOrigin ModuleOrigin(v8::Isolate* isolate,
                              v8::Local<v8::Value> resource_name) {
  return v8::ScriptOrigin(resource_name, v8::Local<v8::Integer>(),
                          v8::Local<v8::Integer>(), v8::Local<v8::Boolean>(),
                          v8::Local<v8::Integer>(), v8::Local<v8::Value>(),
                          v8::Local<v8::Boolean>(), v8::Local<v8::Boolean>(),
                          v8::True(isolate));
}

static inline Local<String> InternalizeStr(Isolate* isolate, const char* x) {
  return String::NewFromUtf8(isolate, x, v8::NewStringType::kInternalized)
      .ToLocalChecked();
}

void ModuleSet::Reset() {
  resolve_ids_.clear();
  by_filename_.clear();
  for (auto it = mods_.begin(); it != mods_.end(); it++) {
    it->second.handle.Reset();
  }
  mods_.clear();
}

ModuleInfo* ModuleSet::Info(deno_mod id) {
  auto it = mods_.find(id);
  CHECK(it != mods_.end());
  return &it->second;
}

deno_mod ModuleSet::Create(Local<Context> context, void* user_data,
                           const char* filename_cstr, const char* source_cstr) {
  auto* isolate = context->GetIsolate();

  v8::Locker locker(isolate);
  Isolate::Scope isolate_scope(isolate);
  EscapableHandleScope handle_scope(isolate);

  // CHECK_EQ(by_filename_.count(filename_cstr), 0);
  auto it = by_filename_.find(filename_cstr);
  if (it != by_filename_.end()) {
    return it->second;
  }

  Local<String> filename_str = InternalizeStr(isolate, filename_cstr);
  Local<String> source_str = InternalizeStr(isolate, source_cstr);

  auto origin = ModuleOrigin(isolate, filename_str);
  ScriptCompiler::Source source(source_str, origin);

  v8::TryCatch try_catch(isolate);

  auto maybe_module = ScriptCompiler::CompileModule(isolate, &source);

  if (try_catch.HasCaught()) {
    CHECK(maybe_module.IsEmpty());
    HandleException(context, try_catch.Exception());
    return 0;
  }

  auto module = maybe_module.ToLocalChecked();

  deno_mod id = module->GetIdentityHash();
  int len = module->GetModuleRequestsLength();

  mods_.emplace(std::piecewise_construct, std::make_tuple(id),
                std::make_tuple(isolate, module, filename_cstr, len));
  by_filename_[filename_cstr] = id;

  auto builtin_modules = GetBuiltinModules(context);

  for (int i = 0; i < len; ++i) {
    Local<String> specifier = module->GetModuleRequest(i);
    v8::String::Utf8Value specifier_utf8(isolate, specifier);
    std::string specifier_str(*specifier_utf8);

    auto* info = Info(id);

    auto resolve_id = next_resolve_id_++;
    resolve_ids_[resolve_id] = PendingResolution{id, i};
    info->children[i] = ChildModule{false, false, 0, specifier_str};

    bool has_builtin = builtin_modules->Has(context, specifier).ToChecked();
    if (has_builtin) {
      deno_mod child_id = CreateBuiltinModule(context, specifier);
      Resolve(context, resolve_id, child_id);
    } else {
      const char* referrer_name = filename_cstr;
      CHECK(resolve_cb_);
      resolve_cb_(user_data, resolve_id, 0, *specifier_utf8, referrer_name, id);
    }
  }

  return id;
}

void ModuleSet::MaybeInstantiate(Local<Context> context, deno_mod id) {
  if (ShouldInstanciate(id)) {
    Instantiate(context, id);
  }
}

bool ModuleSet::HasChildError(deno_mod id) {
  auto* info = Info(id);
  for (auto c : info->children) {
    if (c.resolve_error) {
      return true;
    }
  }
  return false;
}

bool ModuleSet::ShouldInstanciate(deno_mod id) {
  auto* info = Info(id);
  for (auto c : info->children) {
    if (c.child == 0) {
      return false;
    }
  }
  return true;
}

void ModuleSet::Resolve(Local<Context> context, uint32_t resolve_id,
                        deno_mod child_id) {
  auto r = resolve_ids_[resolve_id];
  auto* referrer_info = Info(r.referrer);
  auto c = &referrer_info->children[r.index];
  c->resolved = true;
  if (child_id == 0) {
    c->child = 1;
    c->resolve_error = true;
  } else {
    c->child = child_id;
  }
}

deno_mod ModuleSet::CreateBuiltinModule(Local<Context> context,
                                        Local<String> specifier) {
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::EscapableHandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  v8::String::Utf8Value specifier_utf8val(isolate, specifier);
  const char* specifier_cstr = ToCString(specifier_utf8val);

  // For CreateBuiltinModule specifier is complete. EG "deno"
  // So we don't have to worry about resolving it.
  auto it = by_filename_.find(specifier_cstr);
  if (it != by_filename_.end()) {
    return it->second;
  }

  auto builtin_modules = GetBuiltinModules(context);
  auto val = builtin_modules->Get(context, specifier).ToLocalChecked();
  CHECK(val->IsObject());
  auto obj = val->ToObject(isolate);

  CHECK_EQ(by_filename_.count(specifier_cstr), 0);

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
  return Create(context, nullptr, specifier_cstr, src.c_str());
}

v8::MaybeLocal<v8::Module> ResolveCallback(Local<Context> context,
                                           Local<String> specifier,
                                           Local<Module> referrer) {
  auto* isolate = context->GetIsolate();

  v8::Isolate::Scope isolate_scope(isolate);
  v8::EscapableHandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  CHECK_GT(context->GetNumberOfEmbedderDataFields(), 0);

  auto* ms = static_cast<ModuleSet*>(
      context->GetAlignedPointerFromEmbedderData(kModuleSet));
  CHECK_NE(ms, nullptr);

  auto id = referrer->GetIdentityHash();

  auto* referrer_info = ms->Info(id);

  for (int i = 0; i < referrer->GetModuleRequestsLength(); i++) {
    Local<String> req = referrer->GetModuleRequest(i);
    if (req->Equals(context, specifier).ToChecked()) {
      auto c = ms->Info(id)->children[i];

      if (c.resolve_error || c.child == 0) {
        // Throw exception...  Problem is that we don't get the location of the
        // import?
        char buf[1024];

        v8::String::Utf8Value specifier_utf8val(isolate, specifier);
        const char* specifier_cstr = ToCString(specifier_utf8val);

        const char* referrer_cstr = referrer_info->filename.c_str();

        snprintf(buf, 1024, "Cannot resolve module \"%s\" from \"%s\"",
                 specifier_cstr, referrer_cstr);
        auto exception_str = v8_str(buf, true);
        isolate->ThrowException(exception_str);
        return Local<Module>();
      }

      deno_mod child = c.child;
      Local<Module> child_mod = ms->Info(child)->handle.Get(isolate);
      return handle_scope.Escape(child_mod);
    }
  }
  CHECK(false);  // Unreachable.
  return Local<Module>();
}

void ModuleSet::Instantiate(Local<Context> context, deno_mod id) {
  auto* isolate = context->GetIsolate();
  HandleScope handle_scope(isolate);

  auto* info = Info(id);
  auto module = info->handle.Get(isolate);

  context->SetAlignedPointerInEmbedderData(kModuleSet, this);

  v8::TryCatch try_catch(isolate);

  auto maybe_ok = module->InstantiateModule(context, ResolveCallback);

  CHECK_EQ(this, context->GetAlignedPointerFromEmbedderData(kModuleSet));
  context->SetAlignedPointerInEmbedderData(kModuleSet, nullptr);

  if (try_catch.HasCaught()) {
    DCHECK(maybe_ok.IsNothing());
    HandleException(context, try_catch.Exception());
  } else if (maybe_ok.IsNothing()) {
    CHECK_EQ(Module::kErrored, module->GetStatus());
    HandleException(context, module->GetException());
  }
}

deno_mod_state ModuleSet::State(v8::Local<v8::Context> context, deno_mod id) {
  auto* isolate = context->GetIsolate();
  auto module = Info(id)->handle.Get(isolate);
  if (HasChildError(id)) {
    return DENO_MOD_ERROR;
  }
  switch (module->GetStatus()) {
    case Module::Status::kUninstantiated:
      return DENO_MOD_UNINSTANCIATED;
    case Module::Status::kInstantiating:
      return DENO_MOD_UNINSTANCIATED;
    case Module::Status::kInstantiated:
      return DENO_MOD_INSTANCIATED;
    case Module::Status::kEvaluating:
      return DENO_MOD_INSTANCIATED;
    case Module::Status::kEvaluated:
      return DENO_MOD_EVALUATED;
    case Module::Status::kErrored:
      return DENO_MOD_ERROR;
    default:
      CHECK(false);
  }
}

void ModuleSet::Evaluate(Local<Context> context, void* user_data, deno_mod id) {
  auto* isolate = context->GetIsolate();
  HandleScope handle_scope(isolate);

  MaybeInstantiate(context, id);

  auto module = Info(id)->handle.Get(isolate);

  if (Module::kInstantiated != module->GetStatus()) {
    return;
  }

  auto maybe_result = module->Evaluate(context);
  if (maybe_result.IsEmpty()) {
    CHECK_EQ(Module::kErrored, module->GetStatus());
    HandleException(context, module->GetException());
  }
}

}  // namespace deno
