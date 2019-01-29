
namespace deno {

std::string EncodeMessageAsJSON(v8::Local<v8::Context> context,
                                v8::Local<v8::Message> message) {
  auto* isolate = context->GetIsolate();
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto stack_trace = message->GetStackTrace();

  // Encode the exception into a JS object, which we will then turn into JSON.
  auto json_obj = v8::Object::New(isolate);
  auto exception_str = message->Get();
  CHECK(json_obj->Set(context, v8_str("message"), exception_str).FromJust());

  auto maybe_source_line = message->GetSourceLine(context);
  if (!maybe_source_line.IsEmpty()) {
    CHECK(json_obj
              ->Set(context, v8_str("sourceLine"),
                    maybe_source_line.ToLocalChecked())
              .FromJust());
  }

  CHECK(json_obj
            ->Set(context, v8_str("scriptResourceName"),
                  message->GetScriptResourceName())
            .FromJust());

  auto maybe_line_number = message->GetLineNumber(context);
  if (maybe_line_number.IsJust()) {
    CHECK(json_obj
              ->Set(context, v8_str("lineNumber"),
                    v8::Integer::New(isolate, maybe_line_number.FromJust()))
              .FromJust());
  }

  CHECK(json_obj
            ->Set(context, v8_str("startPosition"),
                  v8::Integer::New(isolate, message->GetStartPosition()))
            .FromJust());

  CHECK(json_obj
            ->Set(context, v8_str("endPosition"),
                  v8::Integer::New(isolate, message->GetEndPosition()))
            .FromJust());

  CHECK(json_obj
            ->Set(context, v8_str("errorLevel"),
                  v8::Integer::New(isolate, message->ErrorLevel()))
            .FromJust());

  auto maybe_start_column = message->GetStartColumn(context);
  if (maybe_start_column.IsJust()) {
    auto start_column =
        v8::Integer::New(isolate, maybe_start_column.FromJust());
    CHECK(
        json_obj->Set(context, v8_str("startColumn"), start_column).FromJust());
  }

  auto maybe_end_column = message->GetEndColumn(context);
  if (maybe_end_column.IsJust()) {
    auto end_column = v8::Integer::New(isolate, maybe_end_column.FromJust());
    CHECK(json_obj->Set(context, v8_str("endColumn"), end_column).FromJust());
  }

  CHECK(json_obj
            ->Set(context, v8_str("isSharedCrossOrigin"),
                  v8::Boolean::New(isolate, message->IsSharedCrossOrigin()))
            .FromJust());

  CHECK(json_obj
            ->Set(context, v8_str("isOpaque"),
                  v8::Boolean::New(isolate, message->IsOpaque()))
            .FromJust());

  v8::Local<v8::Array> frames;
  if (!stack_trace.IsEmpty()) {
    uint32_t count = static_cast<uint32_t>(stack_trace->GetFrameCount());
    frames = v8::Array::New(isolate, count);

    for (uint32_t i = 0; i < count; ++i) {
      auto frame = stack_trace->GetFrame(isolate, i);
      auto frame_obj = v8::Object::New(isolate);
      CHECK(frames->Set(context, i, frame_obj).FromJust());
      auto line = v8::Integer::New(isolate, frame->GetLineNumber());
      auto column = v8::Integer::New(isolate, frame->GetColumn());
      CHECK(frame_obj->Set(context, v8_str("line"), line).FromJust());
      CHECK(frame_obj->Set(context, v8_str("column"), column).FromJust());
      CHECK(frame_obj
                ->Set(context, v8_str("functionName"), frame->GetFunctionName())
                .FromJust());
      // scriptName can be empty in special conditions e.g. eval
      auto scriptName = frame->GetScriptNameOrSourceURL();
      if (scriptName.IsEmpty()) {
        scriptName = v8_str("<unknown>");
      }
      CHECK(
          frame_obj->Set(context, v8_str("scriptName"), scriptName).FromJust());
      CHECK(frame_obj
                ->Set(context, v8_str("isEval"),
                      v8::Boolean::New(isolate, frame->IsEval()))
                .FromJust());
      CHECK(frame_obj
                ->Set(context, v8_str("isConstructor"),
                      v8::Boolean::New(isolate, frame->IsConstructor()))
                .FromJust());
      CHECK(frame_obj
                ->Set(context, v8_str("isWasm"),
                      v8::Boolean::New(isolate, frame->IsWasm()))
                .FromJust());
    }
  } else {
    // No stack trace. We only have one stack frame of info..
    frames = v8::Array::New(isolate, 1);

    auto frame_obj = v8::Object::New(isolate);
    CHECK(frames->Set(context, 0, frame_obj).FromJust());

    auto line =
        v8::Integer::New(isolate, message->GetLineNumber(context).FromJust());
    auto column =
        v8::Integer::New(isolate, message->GetStartColumn(context).FromJust());

    CHECK(frame_obj->Set(context, v8_str("line"), line).FromJust());
    CHECK(frame_obj->Set(context, v8_str("column"), column).FromJust());
    CHECK(frame_obj
              ->Set(context, v8_str("scriptName"),
                    message->GetScriptResourceName())
              .FromJust());
  }

  CHECK(json_obj->Set(context, v8_str("frames"), frames).FromJust());

  auto json_string = v8::JSON::Stringify(context, json_obj).ToLocalChecked();
  v8::String::Utf8Value json_string_(isolate, json_string);
  return std::string(ToCString(json_string_));
}

std::string EncodeExceptionAsJSON(v8::Local<v8::Context> context,
                                  v8::Local<v8::Value> exception) {
  auto* isolate = context->GetIsolate();
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto message = v8::Exception::CreateMessage(isolate, exception);
  return EncodeMessageAsJSON(context, message);
}

void HandleException(v8::Local<v8::Context> context,
                     v8::Local<v8::Value> exception) {
  v8::Isolate* isolate = context->GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  std::string json_str = EncodeExceptionAsJSON(context, exception);
  CHECK(d != nullptr);
  d->last_exception_ = json_str;
}

void HandleExceptionMessage(v8::Local<v8::Context> context,
                            v8::Local<v8::Message> message) {
  v8::Isolate* isolate = context->GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  std::string json_str = EncodeMessageAsJSON(context, message);
  CHECK(d != nullptr);
  d->last_exception_ = json_str;
}

}  // namespace deno
