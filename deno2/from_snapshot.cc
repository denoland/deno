// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>

#include "v8/include/v8.h"

#include "./deno_internal.h"
#include "include/deno.h"

namespace deno {

#include "natives_deno.cc"
#include "snapshot_deno.cc"

Deno* NewFromSnapshot(void* data, RecvCallback cb) {
  auto natives_blob = *StartupBlob_natives();
  auto snapshot_blob = *StartupBlob_snapshot();

  v8::V8::SetNativesDataBlob(&natives_blob);
  v8::V8::SetSnapshotDataBlob(&snapshot_blob);

  Deno* d = new Deno;
  d->cb = cb;
  d->data = data;
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator =
      v8::ArrayBuffer::Allocator::NewDefaultAllocator();
  params.external_references = external_references;
  v8::Isolate* isolate = v8::Isolate::New(params);
  AddIsolate(d, isolate);

  v8::Isolate::Scope isolate_scope(isolate);
  {
    v8::HandleScope handle_scope(isolate);
    auto context = v8::Context::New(isolate);
    d->context.Reset(d->isolate, context);
  }

  return d;
}

}  // namespace deno

extern "C" {

Deno* deno_new(void* data, RecvCallback cb) {
  return deno::NewFromSnapshot(data, cb);
}
}
