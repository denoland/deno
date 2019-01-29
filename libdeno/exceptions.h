#ifndef EXCEPTIONS_H_
#define EXCEPTIONS_H_

#include "third_party/v8/include/v8.h"

namespace deno {

void HandleException(v8::Local<v8::Context> context,
                     v8::Local<v8::Value> exception);

void HandleExceptionMessage(v8::Local<v8::Context> context,
                            v8::Local<v8::Message> message);
}  // namespace deno

#endif  // EXCEPTIONS_H_
