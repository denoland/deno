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

void handle_timer_start(Deno* d, uint32_t cmd_id, uint32_t timer_id,
                        bool interval, uint32_t delay);
void handle_timer_clear(Deno* d, uint32_t cmd_id, uint32_t timer_id);
void handle_read_file_sync(Deno* d, uint32_t cmd_id, const char* filename);
}  // extern "C"
#endif  // HANDLERS_H_
