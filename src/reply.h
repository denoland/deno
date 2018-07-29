// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// TODO(ry) This library handles parsing and sending Flatbuffers. It's written
// in C++ because flatbuffer support for Rust is not quite there. However, once
// flatbuffers are supported in Rust, all of this code should be ported back to
// Rust.

#ifndef REPLY_H_
#define REPLY_H_

#include <stdint.h>
#include "deno.h"

extern "C" {
// Parse incoming messages with C++ Flatbuffers, call into rust handlers.
void deno_handle_msg_from_js(Deno* d, deno_buf buf);
}
#endif  // REPLY_H_
