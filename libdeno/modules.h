// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef MODULES_H_
#define MODULES_H_

#include <map>
#include <string>
#include "deno.h"
#include "third_party/v8/include/v8.h"

namespace deno {

const int kBuiltinModules = 1;
const int kModuleSet = 2;

struct ChildModule {
  bool resolved;
  bool resolve_error;
  deno_mod child;
  std::string specifier;
};

struct ModuleInfo {
  std::string filename;
  std::vector<ChildModule> children;
  v8::Persistent<v8::Module> handle;

  ModuleInfo(v8::Isolate* isolate, v8::Local<v8::Module> module,
             const char* filename_, size_t num_children_)
      : filename(filename_), children(num_children_) {
    handle.Reset(isolate, module);
  }
};

struct PendingResolution {
  deno_mod referrer;
  uint32_t index;
};

class ModuleSet {
 public:
  ModuleSet(deno_resolve_cb cb) : resolve_cb_(cb), next_resolve_id_(1) {}

  void Reset();
  ModuleInfo* Info(deno_mod id);

  deno_mod Create(v8::Local<v8::Context> context, void* user_data,
                  const char* filename, const char* source);

  void Evaluate(v8::Local<v8::Context> context, void* user_data, deno_mod id);

  deno_mod_state State(v8::Local<v8::Context> context, deno_mod id);

  void Resolve(v8::Local<v8::Context> context, uint32_t resolve_id,
               deno_mod id);
  void Instantiate(v8::Local<v8::Context> context, deno_mod id);
  bool ShouldInstanciate(deno_mod id);
  deno_mod CreateBuiltinModule(v8::Local<v8::Context> context,
                               v8::Local<v8::String> specifier);

  bool HasChildError(deno_mod id);

  std::map<deno_mod, ModuleInfo> mods_;

 private:
  void MaybeInstantiate(v8::Local<v8::Context> context, deno_mod id);

  deno_resolve_cb resolve_cb_;
  uint32_t next_resolve_id_;
  std::map<uint32_t, PendingResolution> resolve_ids_;
  std::map<std::string, deno_mod> by_filename_;
};

v8::Local<v8::Object> GetBuiltinModules(v8::Local<v8::Context> context);

}  // namespace deno

#endif  // MODULES_H_
