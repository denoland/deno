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

Deno* NewFromFileSystem(void* data, deno_recv_cb cb) {
  std::string js_source;
  CHECK(deno::ReadFileToString(BUNDLE_LOCATION, &js_source));

  std::string js_source_map;
  CHECK(deno::ReadFileToString(BUNDLE_MAP_LOCATION, &js_source_map));

  Deno* d = new Deno;
  d->currentArgs = nullptr;
  d->cb = cb;
  d->data = data;
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
    // BUNDLE_LOCATION is absolute so deno_ns can load the bundle independently
    // of the cwd. However for source maps to work, the bundle location relative
    // to the build path must be supplied: BUNDLE_REL_LOCATION.
    InitializeContext(isolate, context, BUNDLE_REL_LOCATION, js_source,
                      &js_source_map);
    d->context.Reset(d->isolate, context);
  }

  return d;
}

}  // namespace deno

extern "C" {
Deno* deno_new(void* data, deno_recv_cb cb) {
  return deno::NewFromFileSystem(data, cb);
}
}
