// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include "include/deno.h"

int main(int argc, char** argv) {
  deno_init();

  Deno* d = deno_new(NULL, NULL);
  bool r = deno_execute(d, "deno_main.js", "denoMain();");
  if (!r) {
    printf("Error! %s\n", deno_last_exception(d));
    exit(1);
  }
  deno_delete(d);
}
