// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include <assert.h>
#include <stdio.h>

#include "v8/include/v8.h"

#include "./deno.h"
#include "natives_deno.cc"
#include "snapshot_deno.cc"

int main(int argc, char** argv) {
  v8_init();

  auto natives_blob = *StartupBlob_natives();
  printf("natives_blob %d bytes\n", natives_blob.raw_size);

  auto snapshot_blob = *StartupBlob_snapshot();
  printf("snapshot_blob %d bytes\n", snapshot_blob.raw_size);

  v8::V8::SetNativesDataBlob(&natives_blob);
  v8::V8::SetSnapshotDataBlob(&snapshot_blob);

  Deno* d = deno_from_snapshot(&snapshot_blob, NULL, NULL);
  int r = deno_load(d, "main2.js", "foo();");
  if (r != 0) {
    printf("Error! %s\n", deno_last_exception(d));
    exit(1);
  }

  const char* v = v8::V8::GetVersion();
  printf("Hello World. V8 version %s\n", v);
}
