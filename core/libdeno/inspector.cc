// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#include "third_party/v8/include/v8-inspector.h"
#include "third_party/v8/include/v8.h"

#include "internal.h"

namespace v8 {

InspectorFrontend::InspectorFrontend(Local<Context> context) {
  isolate_ = context->GetIsolate();
  context_.Reset(isolate_, context);
}

void InspectorFrontend::sendResponse(
    int callId, std::unique_ptr<v8_inspector::StringBuffer> message) {
  Send(message->string());
}

void InspectorFrontend::sendNotification(
    std::unique_ptr<v8_inspector::StringBuffer> message) {
  Send(message->string());
}

void InspectorFrontend::Send(const v8_inspector::StringView& string) {
  v8::HandleScope handle_scope(isolate_);
  int length = static_cast<int>(string.length());
  DCHECK_LT(length, v8::String::kMaxLength);
  Local<String> message =
      (string.is8Bit()
           ? v8::String::NewFromOneByte(
                 isolate_,
                 reinterpret_cast<const uint8_t*>(string.characters8()),
                 v8::NewStringType::kNormal, length)
           : v8::String::NewFromTwoByte(
                 isolate_,
                 reinterpret_cast<const uint16_t*>(string.characters16()),
                 v8::NewStringType::kNormal, length))
          .ToLocalChecked();

  String::Utf8Value utf8(isolate_, message);
  std::string str(*utf8);

  char* cstr = &str[0u];

  deno_->inspector_message_cb_(deno_->hack_, cstr);
}

InspectorClient::InspectorClient(Local<Context> context,
                                 deno::DenoIsolate* deno) {
  isolate_ = context->GetIsolate();
  auto* fe = new InspectorFrontend(context);
  fe->deno_ = deno;
  deno_ = deno;
  channel_.reset(fe);
  inspector_ = v8_inspector::V8Inspector::create(isolate_, this);
  session_ = inspector_->connect(1, channel_.get(), v8_inspector::StringView());
  context->SetAlignedPointerInEmbedderData(kInspectorClientIndex, this);
  inspector_->contextCreated(v8_inspector::V8ContextInfo(
      context, kContextGroupId, v8_inspector::StringView()));

  context_.Reset(isolate_, context);
}

void InspectorClient::runMessageLoopOnPause(int context_group_id) {
  // TODO(mtharrison): Needs protection for nested loop?
  terminated_ = false;
  while (!terminated_) {
    deno_->inspector_block_recv_(deno_->user_data_);
  }
}

v8_inspector::V8InspectorSession* InspectorClient::GetSession(
    Local<Context> context) {
  InspectorClient* inspector_client = static_cast<InspectorClient*>(
      context->GetAlignedPointerFromEmbedderData(kInspectorClientIndex));
  return inspector_client->session_.get();
}

}  // namespace v8
