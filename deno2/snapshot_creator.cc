// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
// Hint: --trace_serializer is a useful debugging flag.
#include "deno_internal.h"
#include "file_util.h"
#include "include/deno.h"
#include "v8/include/v8.h"
#include "v8/src/base/logging.h"

namespace deno {

v8::StartupData SerializeInternalFields(v8::Local<v8::Object> holder, int index,
                                        void* data) {
  DCHECK_EQ(data, nullptr);
  InternalFieldData* embedder_field = static_cast<InternalFieldData*>(
      holder->GetAlignedPointerFromInternalField(index));
  if (embedder_field == nullptr) return {nullptr, 0};
  int size = sizeof(*embedder_field);
  char* payload = new char[size];
  // We simply use memcpy to serialize the content.
  memcpy(payload, embedder_field, size);
  return {payload, size};
}

v8::StartupData MakeSnapshot(const char* js_filename, const char* js_source) {
  auto* creator = new v8::SnapshotCreator(external_references);
  auto* isolate = creator->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  {
    v8::HandleScope handle_scope(isolate);
    auto context = v8::Context::New(isolate);
    InitializeContext(isolate, context, js_filename, js_source);
    creator->SetDefaultContext(context, v8::SerializeInternalFieldsCallback(
                                            SerializeInternalFields, nullptr));
  }

  // Note that using kKeep here will cause segfaults. This is demoed in the
  // "SnapshotBug" test case.
  auto snapshot_blob =
      creator->CreateBlob(v8::SnapshotCreator::FunctionCodeHandling::kClear);

  return snapshot_blob;
}

}  // namespace deno

int main(int argc, char** argv) {
  const char* js_fn = argv[1];
  const char* snapshot_out_cc = argv[2];

  v8::V8::SetFlagsFromCommandLine(&argc, argv, true);

  CHECK_EQ(argc, 3);
  CHECK_NE(js_fn, nullptr);
  CHECK_NE(snapshot_out_cc, nullptr);

  std::string js_source;
  CHECK(deno::ReadFileToString(js_fn, &js_source));

  deno_init();
  auto snapshot_blob = deno::MakeSnapshot(js_fn, js_source.c_str());
  std::string snapshot_str(snapshot_blob.data, snapshot_blob.raw_size);

  CHECK(deno::WriteDataAsCpp("snapshot", snapshot_out_cc, snapshot_str));
}
