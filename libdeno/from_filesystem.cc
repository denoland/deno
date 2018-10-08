// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// This file is used to load the bundle at start for deno_ns.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>

#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

#include "deno.h"
#include "file_util.h"
#include "internal.h"

namespace deno {

Deno* NewFromFileSystem(void* user_data, deno_recv_cb cb) {
  std::string exe_path;
  CHECK(deno::ExePath(&exe_path));
  std::string exe_dir = deno::Dirname(exe_path);  // Always ends with a slash.

  std::string js_source_path = exe_dir + BUNDLE_LOCATION;
  std::string js_source;
  CHECK(deno::ReadFileToString(js_source_path.c_str(), &js_source));

  std::string js_source_map_path = exe_dir + BUNDLE_MAP_LOCATION;
  std::string js_source_map;
  CHECK(deno::ReadFileToString(js_source_map_path.c_str(), &js_source_map));

  Deno* d = new Deno;
  d->currentArgs = nullptr;
  d->cb = cb;
  d->user_data = user_data;
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator =
      v8::ArrayBuffer::Allocator::NewDefaultAllocator();
  v8::Isolate* isolate = v8::Isolate::New(params);
  AddIsolate(d, isolate);

  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  {
    v8::HandleScope handle_scope(isolate);
    auto context = v8::Context::New(isolate);
    // For source maps to work, the bundle location that is passed to
    // InitializeContext must be a relative path.
    InitializeContext(isolate, context, BUNDLE_LOCATION, js_source,
                      &js_source_map);
    d->context.Reset(d->isolate, context);
  }

  return d;
}

}  // namespace deno

extern "C" {
Deno* deno_new(void* user_data, deno_recv_cb cb) {
  return deno::NewFromFileSystem(user_data, cb);
}
}
