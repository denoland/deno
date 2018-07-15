// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#ifndef HANDLERS_H_
#define HANDLERS_H_
extern "C" {
#include <stdint.h>

void handle_code_fetch(uint32_t cmd_id, const char* module_specifier,
                       const char* containing_file);
}
#endif  // HANDLERS_H_
