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

#ifdef DENO_MOCK_RUNTIME
#include "natives_mock_runtime.cc"
#include "snapshot_mock_runtime.cc"
#else
#include "natives_deno.cc"
#include "snapshot_deno.cc"
#endif

std::vector<InternalFieldData*> deserialized_data;

void DeserializeInternalFields(v8::Local<v8::Object> holder, int index,
                               v8::StartupData payload, void* data) {
  assert(data == nullptr);  // TODO(ry) pass Deno* object here.
  if (payload.raw_size == 0) {
    holder->SetAlignedPointerInInternalField(index, nullptr);
    return;
  }
  InternalFieldData* embedder_field = new InternalFieldData{0};
  memcpy(embedder_field, payload.data, payload.raw_size);
  holder->SetAlignedPointerInInternalField(index, embedder_field);
  deserialized_data.push_back(embedder_field);
}

Deno* NewFromSnapshot(void* data, deno_sub_cb cb) {
  auto natives_blob = *StartupBlob_natives();
  auto snapshot_blob = *StartupBlob_snapshot();

  v8::V8::SetNativesDataBlob(&natives_blob);
  v8::V8::SetSnapshotDataBlob(&snapshot_blob);
  v8::DeserializeInternalFieldsCallback(DeserializeInternalFields, nullptr);

  Deno* d = new Deno;
  d->currentArgs = nullptr;
  d->cb = cb;
  d->data = data;
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator =
      v8::ArrayBuffer::Allocator::NewDefaultAllocator();
  params.external_references = external_references;
  v8::Isolate* isolate = v8::Isolate::New(params);
  AddIsolate(d, isolate);

  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  {
    v8::HandleScope handle_scope(isolate);
    auto context =
        v8::Context::New(isolate, nullptr, v8::MaybeLocal<v8::ObjectTemplate>(),
                         v8::MaybeLocal<v8::Value>(),
                         v8::DeserializeInternalFieldsCallback(
                             DeserializeInternalFields, nullptr));
    d->context.Reset(d->isolate, context);
  }

  return d;
}

}  // namespace deno

extern "C" {
Deno* deno_new(void* data, deno_sub_cb cb) {
  return deno::NewFromSnapshot(data, cb);
}
}
