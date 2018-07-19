// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
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
    InitializeContext(isolate, context, BUNDLE_LOCATION, js_source.c_str());
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
