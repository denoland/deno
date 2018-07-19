// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

// TODO(ry) This library handles parsing and sending Flatbuffers. It's written
// in C++ because flatbuffer support for Rust is not quite there. However, once
// flatbuffers are supported in Rust, all of this code should be ported back to
// Rust.

#ifndef REPLY_H_
#define REPLY_H_

#include <stdint.h>
#include "deno.h"

extern "C" {

void deno_reply_null(Deno* d, uint32_t cmd_id);
void deno_reply_error(Deno* d, uint32_t cmd_id, const char* error_msg);

void deno_reply_start(Deno* d, uint32_t cmd_id, int argc, char* argv[],
                      char* cwd);
void deno_reply_code_fetch(Deno* d, uint32_t cmd_id, const char* module_name,
                           const char* filename, const char* source_code,
                           const char* output_code);

// Parse incoming messages with C++ Flatbuffers, call into rust handlers.
void deno_handle_msg_from_js(Deno* d, deno_buf buf);
}
#endif  // REPLY_H_
