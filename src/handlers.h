// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef HANDLERS_H_
#define HANDLERS_H_

#include <stdint.h>
#include "deno.h"

extern "C" {
void handle_code_fetch(Deno* d, uint32_t cmd_id, const char* module_specifier,
                       const char* containing_file);
void handle_code_cache(Deno* d, uint32_t cmd_id, const char* filename,
                       const char* source_code, const char* output_code);
}  // extern "C"
#endif  // HANDLERS_H_
