// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Hint: --trace_serializer is a useful debugging flag.
#include <fstream>
#include "deno.h"
#include "file_util.h"
#include "internal.h"
#include "third_party/v8/include/v8.h"
#include "third_party/v8/src/base/logging.h"

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

v8::StartupData MakeSnapshot(const char* js_filename,
                             const std::string& js_source,
                             const std::string* source_map) {
  auto* creator = new v8::SnapshotCreator(external_references);
  auto* isolate = creator->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  {
    v8::HandleScope handle_scope(isolate);
    auto context = v8::Context::New(isolate);
    InitializeContext(isolate, context, js_filename, js_source, source_map);
    creator->SetDefaultContext(context, v8::SerializeInternalFieldsCallback(
                                            SerializeInternalFields, nullptr));
  }

  auto snapshot_blob =
      creator->CreateBlob(v8::SnapshotCreator::FunctionCodeHandling::kClear);

  return snapshot_blob;
}

}  // namespace deno

int main(int argc, char** argv) {
  const char* snapshot_out_bin = argv[1];
  const char* js_fn = argv[2];
  const char* source_map_fn = argv[3];  // Optional.

  v8::V8::SetFlagsFromCommandLine(&argc, argv, true);

  CHECK_NE(js_fn, nullptr);
  CHECK_NE(snapshot_out_bin, nullptr);

  std::string js_source;
  CHECK(deno::ReadFileToString(js_fn, &js_source));

  std::string source_map;
  if (source_map_fn != nullptr) {
    CHECK_EQ(argc, 4);
    CHECK(deno::ReadFileToString(source_map_fn, &source_map));
  }

  deno_init();
  auto snapshot_blob = deno::MakeSnapshot(
      js_fn, js_source, source_map_fn != nullptr ? &source_map : nullptr);
  std::string snapshot_str(snapshot_blob.data, snapshot_blob.raw_size);

  std::ofstream file_(snapshot_out_bin, std::ios::binary);
  file_ << snapshot_str;
  file_.close();
  return file_.bad();
}
