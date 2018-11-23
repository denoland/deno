// Copyright 2018 the Deno authors. All rights reserved. MIT license.

#include "deno.h"

extern "C" {

extern const char snapshot_start asm("snapshot_start");
extern const char snapshot_end asm("snapshot_end");
asm(".data\n"
    "snapshot_start: .incbin \"gen/snapshot_deno.bin\"\n"
    "snapshot_end:\n"
    ".globl snapshot_start;\n"
    ".globl snapshot_end;");
extern const deno_buf deno_snapshot = {
  nullptr, 0, reinterpret_cast<uint8_t*>(const_cast<char*>(&snapshot_start)),
  static_cast<size_t>(&snapshot_end - &snapshot_start)};

}
