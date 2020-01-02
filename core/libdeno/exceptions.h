// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#ifndef EXCEPTIONS_H_
#define EXCEPTIONS_H_

#include <string>
#include "v8/include/v8.h"

namespace deno {

v8::Local<v8::Object> EncodeExceptionAsObject(v8::Local<v8::Context> context,
                                              v8::Local<v8::Value> exception);

std::string EncodeExceptionAsJSON(v8::Local<v8::Context> context,
                                  v8::Local<v8::Value> exception);

void HandleException(v8::Local<v8::Context> context,
                     v8::Local<v8::Value> exception);

void HandleExceptionMessage(v8::Local<v8::Context> context,
                            v8::Local<v8::Message> message);

void ClearException(v8::Local<v8::Context> context);

void ThrowInvalidArgument(v8::Isolate* isolate);
}  // namespace deno

#endif  // EXCEPTIONS_H_
