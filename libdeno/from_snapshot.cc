// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>

#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

#include "deno.h"
#include "internal.h"

extern const char deno_snapshot_start asm("deno_snapshot_start");
extern const char deno_snapshot_end asm("deno_snapshot_end");
#ifdef LIBDENO_TEST
asm(".data\n"
    "deno_snapshot_start: .incbin \"gen/snapshot_libdeno_test.bin\"\n"
    "deno_snapshot_end:\n"
    ".globl deno_snapshot_start;\n"
    ".globl deno_snapshot_end;");
#else
asm(".data\n"
    "deno_snapshot_start: .incbin \"gen/snapshot_deno.bin\"\n"
    "deno_snapshot_end:\n"
    ".globl deno_snapshot_start;\n"
    ".globl deno_snapshot_end;");
#endif  // LIBDENO_TEST

namespace deno {

std::vector<InternalFieldData*> deserialized_data;

void DeserializeInternalFields(v8::Local<v8::Object> holder, int index,
                               v8::StartupData payload, void* data) {
  DCHECK_EQ(data, nullptr);
  if (payload.raw_size == 0) {
    holder->SetAlignedPointerInInternalField(index, nullptr);
    return;
  }
  InternalFieldData* embedder_field = new InternalFieldData{0};
  memcpy(embedder_field, payload.data, payload.raw_size);
  holder->SetAlignedPointerInInternalField(index, embedder_field);
  deserialized_data.push_back(embedder_field);
}

Deno* NewFromSnapshot(deno_recv_cb cb) {
  Deno* d = new Deno;
  d->currentArgs = nullptr;
  d->cb = cb;
  d->user_data = nullptr;
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator =
      v8::ArrayBuffer::Allocator::NewDefaultAllocator();
  params.external_references = external_references;

  CHECK_NE(&deno_snapshot_start, nullptr);
  int snapshot_len =
      static_cast<int>(&deno_snapshot_end - &deno_snapshot_start);
  static v8::StartupData snapshot = {&deno_snapshot_start, snapshot_len};
  params.snapshot_blob = &snapshot;

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
Deno* deno_new(deno_recv_cb cb) { return deno::NewFromSnapshot(cb); }
}
